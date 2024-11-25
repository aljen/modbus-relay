use std::{future::Future, net::SocketAddr, sync::Arc, time::Duration};

use tokio::{
    io::{AsyncReadExt, AsyncWriteExt, ReadHalf, WriteHalf},
    net::{TcpListener, TcpStream},
    sync::{broadcast, Mutex},
    task::JoinHandle,
    time::{sleep, timeout, Instant},
};
use tracing::{debug, error, info};

use crate::{
    connection_manager::{ConnectionConfig, ConnectionManager},
    errors::{
        ClientErrorKind, ConfigValidationError, ConnectionError, FrameErrorKind, ProtocolErrorKind,
        RelayError,
    },
    generate_request_id,
    relay_config::RelayConfig,
    rtu_transport::RtuTransport,
    IoOperation, ModbusProcessor, TransportError,
};

pub struct ModbusRelay {
    transport: Arc<RtuTransport>,
    config: RelayConfig,
    connection_manager: Arc<ConnectionManager>,
    shutdown: broadcast::Sender<()>,
    main_shutdown: tokio::sync::watch::Sender<bool>,
    tasks: Arc<Mutex<Vec<JoinHandle<()>>>>,
}

impl ModbusRelay {
    pub fn new(config: RelayConfig) -> Result<Self, RelayError> {
        // Validate the config first
        config.validate()?;

        let transport = RtuTransport::new(&config)?;

        let conn_config = ConnectionConfig::default(); // TODO: Add to RelayConfig
        conn_config
            .validate()
            .map_err(|e| RelayError::Config(ConfigValidationError::connection(e.to_string())))?;

        let (main_shutdown, _) = tokio::sync::watch::channel(false);

        Ok(Self {
            transport: Arc::new(transport),
            connection_manager: Arc::new(ConnectionManager::new(conn_config)),
            config,
            shutdown: broadcast::channel(1).0,
            main_shutdown,
            tasks: Arc::new(Mutex::new(Vec::new())),
        })
    }

    fn spawn_task<F>(&self, name: &str, future: F)
    where
        F: Future<Output = ()> + Send + 'static,
    {
        let task = tokio::spawn(future);
        debug!("Spawned {} task: {:?}", name, task.id());

        let _ = self.tasks.try_lock().map(|mut guard| guard.push(task));
    }

    pub async fn run(self: Arc<Self>) -> Result<(), RelayError> {
        let addr = format!(
            "{}:{}",
            self.config.tcp_bind_addr, self.config.tcp_bind_port
        );

        let listener = TcpListener::bind(&addr).await.map_err(|e| {
            RelayError::Transport(TransportError::Io {
                operation: IoOperation::Configure,
                details: format!("Failed to bind to address {}", addr),
                source: e,
            })
        })?;

        info!("Listening on {}", addr);

        // Start a task to clean up idle connections
        let manager = Arc::clone(&self.connection_manager);
        let mut shutdown_rx = self.shutdown.subscribe();

        self.spawn_task("cleanup", async move {
            loop {
                tokio::select! {
                    _ = sleep(Duration::from_secs(60)) => {
                        if let Err(e) = manager.cleanup_idle_connections().await {
                            error!("Error during connection cleanup: {}", e);
                        }
                    }
                    _ = shutdown_rx.recv() => {
                        debug!("Cleanup task received shutdown signal");
                        break;
                    }
                }
            }
        });

        // Periodically log statistics
        let manager = Arc::clone(&self.connection_manager);
        let mut shutdown_rx = self.shutdown.subscribe();

        self.spawn_task("stats", async move {
            loop {
                tokio::select! {
                    _ = sleep(Duration::from_secs(300)) => {
                        match manager.get_stats().await {
                            Ok(stats) => info!("Connection stats: {:?}", stats),
                            Err(e) => error!("Failed to get connection stats: {}", e),
                        }
                    }
                    _ = shutdown_rx.recv() => {
                        debug!("Stats task received shutdown signal");
                        break;
                    }
                }
            }
        });

        let mut shutdown_rx = self.main_shutdown.subscribe();

        loop {
            tokio::select! {
                accept_result = listener.accept() => {
                    match accept_result {
                        Ok((socket, peer)) => {
                            info!("New connection from {}", peer);

                            // Attempt to accept connection by connection manager
                            match self.connection_manager.accept_connection(peer).await {
                                Ok(guard) => {
                                    let transport = Arc::clone(&self.transport);
                                    let manager = Arc::clone(&self.connection_manager);

                                    self.spawn_task("client", async move {
                                        if let Err(e) =
                                            handle_client(socket, transport, &manager, peer).await
                                        {
                                            error!("Client error: {}", e);
                                            // Record failed connection in stats
                                            if let Err(stat_err) =
                                                manager.record_client_error(&peer).await
                                            {
                                                error!("Failed to record client error: {}", stat_err);
                                            }
                                        }
                                        drop(guard); // Explicit drop of guard to ensure cleanup
                                    });
                                }
                                Err(e) => {
                                    error!("Connection rejected: {}", e);
                                    // Add a delay here to slow down connection floods
                                    sleep(Duration::from_millis(100)).await;
                                }
                            }
                        }
                        Err(e) => {
                            error!("Accept error: {}", e);
                            sleep(Duration::from_millis(100)).await;
                        }
                    }

                }
                _ = shutdown_rx.changed() => {
                    info!("Main loop received shutdown signal");
                    break;
                }
            }
        }

        info!("Main loop exited");
        Ok(())
    }

    /// Graceful shutdown
    pub async fn shutdown(&self) -> Result<(), RelayError> {
        info!("Initiating graceful shutdown");
        let timeout_duration = Duration::from_secs(5);

        let _ = self.main_shutdown.send(true);

        // 1. Log initial state
        if let Ok(stats) = self.connection_manager.get_stats().await {
            info!(
                "Current state: {} active connections, {} total requests",
                stats.active_connections, stats.total_requests
            );
        }

        // 2. Sending shutdown signal to all tasks
        info!("Sending shutdown signal to tasks");
        self.shutdown.send(()).map_err(|e| {
            RelayError::Connection(ConnectionError::InvalidState(format!(
                "Failed to send shutdown signal: {}",
                e
            )))
        })?;

        // 3. Initiate connection shutdown
        info!("Initiating connection shutdown");
        if let Err(e) = self.connection_manager.close_all_connections().await {
            error!("Error initiating connection shutdown: {}", e);
        }

        // 4. Wait for connections to close with timeout
        let start = Instant::now();
        loop {
            if start.elapsed() >= timeout_duration {
                error!("Timeout waiting for connections to close");
                break;
            }

            if let Ok(stats) = self.connection_manager.get_stats().await {
                if stats.active_connections == 0 {
                    info!("All connections closed");
                    break;
                }
                info!(
                    "Waiting for {} connections to close",
                    stats.active_connections
                );
            }
            tokio::time::sleep(Duration::from_secs(1)).await;
        }

        // 5. Now we can safely close the serial port
        info!("Closing serial port");
        if let Err(e) = self.transport.close().await {
            error!("Error closing serial port: {}", e);
        }

        // 6. Waiting for all tasks to complete
        info!("Waiting for tasks to complete");
        let tasks = {
            let mut tasks_guard = self.tasks.lock().await;
            tasks_guard.drain(..).collect::<Vec<_>>()
        };

        match tokio::time::timeout(timeout_duration, futures::future::join_all(tasks)).await {
            Ok(results) => {
                let mut failed = 0;
                for (i, result) in results.into_iter().enumerate() {
                    if let Err(e) = result {
                        error!("Task {} failed during shutdown: {}", i, e);
                        failed += 1;
                    }
                }
                if failed > 0 {
                    error!("{} tasks failed during shutdown", failed);
                } else {
                    info!("All tasks completed successfully");
                }
            }
            Err(_) => {
                error!(
                    "Timeout waiting for tasks to complete after {:?}",
                    timeout_duration
                );
            }
        }

        info!("Shutdown complete");
        Ok(())
    }
}

async fn read_frame(
    reader: &mut tokio::net::tcp::ReadHalf<'_>,
    peer_addr: SocketAddr,
    manager: &ConnectionManager,
) -> Result<(Vec<u8>, [u8; 2]), RelayError> {
    let mut tcp_buf = vec![0u8; 256];

    // Read TCP request with timeout
    let n = match timeout(Duration::from_secs(60), reader.read(&mut tcp_buf)).await {
        Ok(Ok(0)) => {
            return Err(RelayError::Connection(ConnectionError::Disconnected));
        }
        Ok(Ok(n)) => {
            if n < 7 {
                manager.record_request(peer_addr, false).await;
                return Err(RelayError::frame(
                    FrameErrorKind::TooShort,
                    format!("Frame too short: {} bytes", n),
                    Some(tcp_buf[..n].to_vec()),
                ));
            }
            n
        }
        Ok(Err(e)) => {
            manager.record_request(peer_addr, false).await;
            return Err(RelayError::Connection(ConnectionError::InvalidState(
                format!("Connection lost: {}", e),
            )));
        }
        Err(_) => {
            manager.record_request(peer_addr, false).await;
            return Err(RelayError::Connection(ConnectionError::Timeout(
                "Read operation timed out".to_string(),
            )));
        }
    };

    debug!(
        "Received TCP frame from {}: {:02X?}",
        peer_addr,
        &tcp_buf[..n]
    );

    // Validate MBAP header
    let transaction_id = [tcp_buf[0], tcp_buf[1]];
    let protocol_id = u16::from_be_bytes([tcp_buf[2], tcp_buf[3]]);
    if protocol_id != 0 {
        manager.record_request(peer_addr, false).await;
        return Err(RelayError::protocol(
            ProtocolErrorKind::InvalidProtocolId,
            format!("Invalid protocol ID: {}", protocol_id),
        ));
    }

    let length = u16::from_be_bytes([tcp_buf[4], tcp_buf[5]]) as usize;
    if length > 249 {
        manager.record_request(peer_addr, false).await;
        return Err(RelayError::frame(
            FrameErrorKind::TooLong,
            format!("Frame too long: {} bytes", length),
            None,
        ));
    }

    if length + 6 != n {
        manager.record_request(peer_addr, false).await;
        return Err(RelayError::frame(
            FrameErrorKind::InvalidFormat,
            format!("Invalid frame length, expected {}, got {}", length + 6, n),
            Some(tcp_buf[..n].to_vec()),
        ));
    }

    Ok((tcp_buf[..n].to_vec(), transaction_id))
}

async fn process_frame(
    modbus: &ModbusProcessor,
    frame: &[u8],
    transaction_id: [u8; 2],
) -> Result<Vec<u8>, RelayError> {
    modbus
        .process_request(
            transaction_id,
            frame[6],     // Unit ID
            &frame[7..], // PDU
        )
        .await
}

async fn send_response(
    writer: &mut tokio::net::tcp::WriteHalf<'_>,
    response: &[u8],
    peer_addr: SocketAddr,
) -> Result<(), RelayError> {
    debug!("Sending TCP response to {}: {:02X?}", peer_addr, response);

    // Send TCP response with timeout
    match timeout(Duration::from_secs(5), writer.write_all(response)).await {
        Ok(Ok(_)) => Ok(()),
        Ok(Err(e)) => Err(RelayError::client(
            ClientErrorKind::WriteError,
            peer_addr,
            format!("Write error: {}", e),
        )),
        Err(_) => Err(RelayError::client(
            ClientErrorKind::Timeout,
            peer_addr,
            "Write timeout".to_string(),
        )),
    }
}

async fn handle_client(
    mut socket: TcpStream,
    transport: Arc<RtuTransport>,
    manager: &ConnectionManager,
    peer_addr: SocketAddr,
) -> Result<(), RelayError> {
    let request_id = generate_request_id();

    let client_span = tracing::info_span!(
        "client_connection",
        %peer_addr,
        request_id = %request_id,
        protocol = "modbus_tcp"
    );
    let _enter = client_span.enter();

    socket.set_nodelay(true).map_err(|e| {
        RelayError::Transport(TransportError::Io {
            operation: IoOperation::Configure,
            details: "Failed to set TCP_NODELAY".to_string(),
            source: e,
        })
    })?;

    let addr = socket.peer_addr().map_err(|e| {
        RelayError::Transport(TransportError::Io {
            operation: IoOperation::Control,
            details: "Failed to get peer address".to_string(),
            source: e,
        })
    })?;

    info!("New client connected from {}", addr);

    let (mut reader, mut writer) = socket.split();
    let modbus = ModbusProcessor::new(transport);

    loop {
        // 1. Read frame
        let (frame, transaction_id) = match read_frame(&mut reader, peer_addr, manager).await {
            Ok((frame, id)) => (frame, id),
            Err(RelayError::Connection(ConnectionError::Disconnected)) => {
                info!("Client {} disconnected", peer_addr);
                break;
            }
            Err(e) => return Err(e),
        };

        // 2. Process frame
        let response = match process_frame(&modbus, &frame, transaction_id).await {
            Ok(response) => response,
            Err(e) => {
                manager.record_request(peer_addr, false).await;
                return Err(e);
            }
        };

        // 3. Send response
        if let Err(e) = send_response(&mut writer, &response, peer_addr).await {
            manager.record_request(peer_addr, false).await;
            return Err(e);
        }

        manager.record_request(peer_addr, true).await;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_modbus_relay_shutdown() {
        let config = RelayConfig::default();
        let relay = ModbusRelay::new(config).unwrap();

        assert!(relay.shutdown().await.is_ok());
    }
}
