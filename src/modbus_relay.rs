use std::{future::Future, net::SocketAddr, sync::Arc, time::Duration, time::Instant};

use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::{broadcast, mpsc, Mutex},
    task::JoinHandle,
    time::{sleep, timeout},
};
use tracing::{debug, error, info, warn};

use crate::{
    connection::StatEvent,
    errors::{
        ClientErrorKind, ConnectionError, FrameErrorKind, ProtocolErrorKind, RelayError,
        TransportError,
    },
    http_api::start_http_server,
    rtu_transport::RtuTransport,
    utils::generate_request_id,
    ConnectionManager, IoOperation, ModbusProcessor, RelayConfig, StatsConfig, StatsManager,
};

use socket2::{SockRef, TcpKeepalive};

pub struct ModbusRelay {
    config: RelayConfig,
    transport: Arc<RtuTransport>,
    connection_manager: Arc<ConnectionManager>,
    stats_manager: Arc<Mutex<StatsManager>>,
    stats_tx: mpsc::Sender<StatEvent>,
    shutdown: broadcast::Sender<()>,
    main_shutdown: tokio::sync::watch::Sender<bool>,
    tasks: Arc<Mutex<Vec<JoinHandle<()>>>>,
}

impl ModbusRelay {
    pub fn new(config: RelayConfig) -> Result<Self, RelayError> {
        // Validate the config first
        RelayConfig::validate(&config)?;

        let transport = RtuTransport::new(&config.rtu, config.logging.trace_frames)?;

        // Create stats manager first
        let stats_config = StatsConfig {
            cleanup_interval: config.connection.idle_timeout,
            idle_timeout: config.connection.idle_timeout,
            error_timeout: config.connection.error_timeout,
            max_events_per_second: 10000, // TODO(aljen): Make configurable
        };
        let (stats_manager, stats_tx) = StatsManager::new(stats_config);
        let stats_manager = Arc::new(Mutex::new(stats_manager));

        // Initialize connection manager with stats sender
        let connection_manager = Arc::new(ConnectionManager::new(
            config.connection.clone(),
            stats_tx.clone(),
        ));

        let (shutdown_tx, _) = broadcast::channel(1);
        let (main_shutdown_tx, _) = tokio::sync::watch::channel(false);

        Ok(Self {
            config,
            transport: Arc::new(transport),
            connection_manager,
            stats_manager,
            stats_tx,
            shutdown: shutdown_tx,
            main_shutdown: main_shutdown_tx,
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
        // Start stats manager
        let shutdown_rx = self.shutdown.subscribe();

        self.spawn_task("stats_manager", {
            let stats_manager = Arc::clone(&self.stats_manager);

            async move {
                let mut stats_manager = stats_manager.lock().await;
                stats_manager.run(shutdown_rx).await;
            }
        });

        // Start TCP server
        let tcp_server = {
            let transport = Arc::clone(&self.transport);
            let manager = Arc::clone(&self.connection_manager);
            let stats_tx = self.stats_tx.clone();
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
                                    let stats_tx = stats_tx.clone();

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
                                        if let Err(e) = handle_client(socket, peer, transport, manager, stats_tx).await {
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

        self.spawn_task("tcp_server", async move {
            if let Err(e) = tcp_server.await {
                error!("TCP server task failed: {}", e);
            }
        });

        // Start HTTP server if enabled
        if self.config.http.enabled {
            let http_server = start_http_server(
                self.config.http.bind_addr.clone(),
                self.config.http.bind_port,
                self.connection_manager.clone(),
                self.shutdown.subscribe(),
            );

            self.spawn_task("http", async move {
                if let Err(e) = http_server.await {
                    error!("HTTP server error: {}", e)
                }
            });
        }

        // Start a task to clean up idle connections
        let manager = Arc::clone(&self.connection_manager);
        let mut shutdown_rx = self.shutdown.subscribe();

        self.spawn_task("cleanup", async move {
            let mut interval = tokio::time::interval(Duration::from_secs(60));

            loop {
                tokio::select! {
                    _ = interval.tick() => {
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

        // Wait for shutdown signal
        let mut shutdown_rx = self.main_shutdown.subscribe();

        tokio::select! {
            _ = shutdown_rx.changed() => {
                info!("Main loop received shutdown signal");
            }
        }

        info!("Main loop exited");
        Ok(())
    }

    /// Graceful shutdown
    pub async fn shutdown(&self) -> Result<(), RelayError> {
        info!("Initiating graceful shutdown");
        let timeout_duration = Duration::from_secs(5);

        // Send main shutdown signal
        let _ = self.main_shutdown.send(true);

        // 1. Log initial state
        let stats = self.connection_manager.get_stats().await?;
        info!(
            "Current state: {} active connections, {} total requests",
            stats.active_connections, stats.total_requests
        );

        // 2. Send shutdown signal to all tasks
        info!("Sending shutdown signal to tasks");
        self.shutdown.send(()).map_err(|e| {
            RelayError::Connection(ConnectionError::invalid_state(format!(
                "Failed to send shutdown signal: {}",
                e
            )))
        })?;

        // 3. Initiate connection shutdown
        info!("Initiating connection shutdown");
        if let Err(e) = self.connection_manager.close_all_connections().await {
            error!("Error during connection shutdown: {}", e);
        }

        // 4. Wait for connections to close with timeout
        let start = Instant::now();
        while start.elapsed() < timeout_duration {
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
            sleep(Duration::from_secs(1)).await;
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
) -> Result<(Vec<u8>, [u8; 2]), RelayError> {
    let mut tcp_buf = vec![0u8; 256];

    // Read TCP request with timeout
    let n = match timeout(Duration::from_secs(60), reader.read(&mut tcp_buf)).await {
        Ok(Ok(0)) => {
            return Err(RelayError::Connection(ConnectionError::Disconnected));
        }
        Ok(Ok(n)) => {
            if n < 7 {
                return Err(RelayError::frame(
                    FrameErrorKind::TooShort,
                    format!("Frame too short: {} bytes", n),
                    Some(tcp_buf[..n].to_vec()),
                ));
            }
            n
        }
        Ok(Err(e)) => {
            return Err(RelayError::Connection(ConnectionError::InvalidState(
                format!("Connection lost: {}", e),
            )));
        }
        Err(_) => {
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
        return Err(RelayError::protocol(
            ProtocolErrorKind::InvalidProtocolId,
            format!("Invalid protocol ID: {}", protocol_id),
        ));
    }

    let length = u16::from_be_bytes([tcp_buf[4], tcp_buf[5]]) as usize;
    if length > 249 {
        return Err(RelayError::frame(
            FrameErrorKind::TooLong,
            format!("Frame too long: {} bytes", length),
            None,
        ));
    }

    if length + 6 != n {
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
    mut stream: TcpStream,
    peer_addr: SocketAddr,
    transport: Arc<RtuTransport>,
    manager: Arc<ConnectionManager>,
    stats_tx: mpsc::Sender<StatEvent>,
) -> Result<(), RelayError> {
    // Create connection guard to track this connection
    let _guard = manager.accept_connection(peer_addr).await?;

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
        let frame_start = Instant::now();

        // 1. Read frame
        let (frame, transaction_id) = match read_frame(&mut reader, peer_addr).await {
            Ok((frame, id)) => (frame, id),
            Err(RelayError::Connection(ConnectionError::Disconnected)) => {
                info!("Client {} disconnected", peer_addr);
                break;
            }
            Err(e) => {
                stats_tx
                    .send(StatEvent::RequestProcessed {
                        addr: peer_addr,
                        success: false,
                        duration_ms: frame_start.elapsed().as_millis() as u64,
                    })
                    .await
                    .map_err(|e| {
                        warn!("Failed to send stats event: {}", e);
                    })
                    .ok();

                return Err(e);
            }
        };

        // 2. Process frame
        let response = match process_frame(&modbus, &frame, transaction_id).await {
            Ok(response) => {
                // Record successful Modbus request
                stats_tx
                    .send(StatEvent::RequestProcessed {
                        addr: peer_addr,
                        success: true,
                        duration_ms: frame_start.elapsed().as_millis() as u64,
                    })
                    .await
                    .map_err(|e| {
                        warn!("Failed to send stats event: {}", e);
                    })
                    .ok();

                response
            }
            Err(e) => {
                // Record failed Modbus request
                stats_tx
                    .send(StatEvent::RequestProcessed {
                        addr: peer_addr,
                        success: false,
                        duration_ms: frame_start.elapsed().as_millis() as u64,
                    })
                    .await
                    .map_err(|e| {
                        warn!("Failed to send stats event: {}", e);
                    })
                    .ok();

                return Err(e);
            }
        };

        // 3. Send response
        if let Err(e) = send_response(&mut writer, &response, peer_addr).await {
            stats_tx
                .send(StatEvent::RequestProcessed {
                    addr: peer_addr,
                    success: false,
                    duration_ms: frame_start.elapsed().as_millis() as u64,
                })
                .await
                .map_err(|e| {
                    warn!("Failed to send stats event: {}", e);
                })
                .ok();

            return Err(e);
        }
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
