use std::{future::Future, net::SocketAddr, sync::Arc, time::Duration, time::Instant};

use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::{broadcast, Mutex},
    task::JoinHandle,
    time::{sleep, timeout},
};
use tracing::{debug, error, info};

use crate::errors::{
    ClientErrorKind, ConnectionError, FrameErrorKind, ProtocolErrorKind, RelayError, TransportError,
};
use crate::http_api::start_http_server;
use crate::rtu_transport::RtuTransport;
use crate::utils::generate_request_id;
use crate::{ConnectionManager, IoOperation, ModbusProcessor, RelayConfig};

use socket2::{SockRef, TcpKeepalive};

pub struct ModbusRelay {
    config: RelayConfig,
    transport: Arc<RtuTransport>,
    connection_manager: Arc<ConnectionManager>,
    shutdown: broadcast::Sender<()>,
    main_shutdown: tokio::sync::watch::Sender<bool>,
    tasks: Arc<Mutex<Vec<JoinHandle<()>>>>,
}

impl ModbusRelay {
    pub fn new(config: RelayConfig) -> Result<Self, RelayError> {
        // Validate the config first
        RelayConfig::validate(&config)?;

        let transport = RtuTransport::new(&config.rtu, config.logging.trace_frames)?;

        // Initialize connection manager with connection config from RelayConfig
        let connection_manager = Arc::new(ConnectionManager::new(config.connection.clone()));

        let (main_shutdown, _) = tokio::sync::watch::channel(false);

        Ok(Self {
            config,
            transport: Arc::new(transport),
            connection_manager,
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

    async fn configure_tcp_stream(
        socket: &TcpStream,
        keep_alive_duration: Duration,
    ) -> Result<(), RelayError> {
        // Configure TCP socket using SockRef
        let sock_ref = SockRef::from(&socket);

        // Enable TCP keepalive
        sock_ref.set_keepalive(true).map_err(|e| {
            RelayError::Transport(TransportError::Io {
                operation: IoOperation::Configure,
                details: "Failed to enable TCP keepalive".to_string(),
                source: e,
            })
        })?;

        // Set TCP_NODELAY
        sock_ref.set_nodelay(true).map_err(|e| {
            RelayError::Transport(TransportError::Io {
                operation: IoOperation::Configure,
                details: "Failed to set TCP_NODELAY".to_string(),
                source: e,
            })
        })?;

        #[cfg(any(target_os = "linux", target_os = "macos"))]
        {
            let mut ka = TcpKeepalive::new();
            ka = ka.with_time(keep_alive_duration);
            ka = ka.with_interval(keep_alive_duration);

            sock_ref.set_tcp_keepalive(&ka).map_err(|e| {
                RelayError::Transport(TransportError::Io {
                    operation: IoOperation::Configure,
                    details: "Failed to set TCP keepalive parameters".to_string(),
                    source: e,
                })
            })?;
        }

        Ok(())
    }

    pub async fn run(self: Arc<Self>) -> Result<(), RelayError> {
        let mut tasks = Vec::new();

        // Start TCP server
        let tcp_server = {
            let transport = Arc::clone(&self.transport);
            let manager = Arc::clone(&self.connection_manager);
            let mut rx = self.shutdown.subscribe();
            let config = self.config.clone();
            let keep_alive_duration = self.config.tcp.keep_alive;

            tokio::spawn(async move {
                let addr = format!("{}:{}", config.tcp.bind_addr, config.tcp.bind_port);
                let listener = TcpListener::bind(&addr).await.map_err(|e| {
                    RelayError::Transport(TransportError::Io {
                        operation: IoOperation::Listen,
                        details: format!("Failed to bind TCP listener to {}", addr),
                        source: e,
                    })
                })?;

                info!("MODBUS TCP server listening on {}", addr);

                loop {
                    tokio::select! {
                        accept_result = listener.accept() => {
                            match accept_result {
                                Ok((socket, peer)) => {
                                    let transport = Arc::clone(&transport);
                                    let manager = Arc::clone(&manager);

                                    Self::configure_tcp_stream(&socket, keep_alive_duration)
                                        .await
                                        .map_err(|e| {
                                            error!("Failed to configure TCP stream: {}", e);
                                        })
                                        .map(|_| {
                                            debug!(
                                                "TCP stream configured with keepalive: {:?}",
                                                keep_alive_duration
                                            )
                                        })
                                        .ok();

                                    tokio::spawn(async move {
                                        if let Err(e) = handle_client(socket, peer, transport, manager).await {
                                            error!("Client error: {}", e);
                                        }
                                    });
                                }
                                Err(e) => {
                                    error!("Failed to accept connection: {}", e);
                                }
                            }
                        }
                        _ = rx.recv() => {
                            info!("MODBUS TCP server shutting down");
                            break;
                        }
                    }
                }

                Ok::<_, RelayError>(())
            })
        };
        tasks.push(tcp_server);

        // Start HTTP server if enabled
        if self.config.http.enabled {
            let http_server = {
                let manager = Arc::clone(&self.connection_manager);
                let rx = self.shutdown.subscribe();
                let config = self.config.clone();

                tokio::spawn(async move {
                    if let Err(e) =
                        start_http_server(config.http.bind_addr, config.http.bind_port, manager, rx)
                            .await
                    {
                        error!("HTTP server error: {}", e);
                        return Err(RelayError::Transport(TransportError::Io {
                            operation: IoOperation::Listen,
                            details: format!("HTTP server error: {}", e),
                            source: std::io::Error::new(std::io::ErrorKind::Other, e.to_string()),
                        }));
                    }
                    Ok(())
                })
            };
            tasks.push(http_server);
        }

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

        tokio::select! {
            _ = shutdown_rx.changed() => {
                info!("Main loop received shutdown signal");
            }
        }

        // Wait for all tasks to complete
        for task in tasks {
            if let Err(e) = task.await {
                error!("Task error: {}", e);
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
                    if result.is_err() {
                        error!("Task {} failed during shutdown: {}", i, result.unwrap_err());
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
            frame[6],    // Unit ID
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
    stream: TcpStream,
    peer_addr: SocketAddr,
    transport: Arc<RtuTransport>,
    manager: Arc<ConnectionManager>,
) -> Result<(), RelayError> {
    let start_time = Instant::now();

    // Create connection guard to track this connection
    let _guard = manager.accept_connection(peer_addr).await?;

    let result = handle_client_inner(stream, peer_addr, transport, manager.clone()).await;

    if result.is_err() {
        manager.record_client_error(&peer_addr).await?;
    }

    manager.record_response_time(start_time.elapsed()).await;

    result
}

async fn handle_client_inner(
    mut stream: TcpStream,
    peer_addr: SocketAddr,
    transport: Arc<RtuTransport>,
    manager: Arc<ConnectionManager>,
) -> Result<(), RelayError> {
    let request_id = generate_request_id();

    let client_span = tracing::info_span!(
        "client_connection",
        %peer_addr,
        request_id = %request_id,
        protocol = "modbus_tcp"
    );
    let _enter = client_span.enter();

    let addr = stream.peer_addr().map_err(|e| {
        RelayError::Transport(TransportError::Io {
            operation: IoOperation::Control,
            details: "Failed to get peer address".to_string(),
            source: e,
        })
    })?;

    info!("New client connected from {}", addr);

    let (mut reader, mut writer) = stream.split();
    let modbus = ModbusProcessor::new(transport);

    loop {
        // 1. Read frame
        let (frame, transaction_id) = match read_frame(&mut reader, peer_addr, &manager).await {
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
                return Err(e);
            }
        };

        // 3. Send response
        send_response(&mut writer, &response, peer_addr).await?
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    // use super::*;

    // #[tokio::test]
    // Disabled for now, needs port mocking
    // async fn test_modbus_relay_shutdown() {
    //     let mut config = RelayConfig::default();
    //     config.rtu.device = "/dev/null".to_string();
    //     let relay = ModbusRelay::new(config).unwrap();

    //     assert!(relay.shutdown().await.is_ok());
    // }
}
