use std::{future::Future, net::SocketAddr, sync::Arc, time::Duration};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::broadcast,
    time::{sleep, timeout},
};
use tracing::{debug, error, info};

use crate::{
    connection_manager::{ConnectionConfig, ConnectionManager},
    errors::{
        ClientErrorKind, ConfigValidationError, ConnectionError, FrameErrorKind, ProtocolErrorKind,
        RelayError,
    },
    relay_config::RelayConfig,
    rtu_transport::RtuTransport,
    IoOperation, ModbusProcessor, TransportError,
};

pub struct ModbusRelay {
    transport: Arc<RtuTransport>,
    config: RelayConfig,
    connection_manager: Arc<ConnectionManager>,
    shutdown: broadcast::Sender<()>,
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

        Ok(Self {
            transport: Arc::new(transport),
            connection_manager: Arc::new(ConnectionManager::new(conn_config)),
            config,
            shutdown: broadcast::channel(1).0,
        })
    }

    fn spawn_task<F>(&self, name: &str, future: F)
    where
        F: Future<Output = ()> + Send + 'static,
    {
        let task = tokio::spawn(future);
        debug!("Spawned {} task: {:?}", name, task.id());
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

        loop {
            let accept_result = listener.accept().await;
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
                                    if let Err(stat_err) = manager.record_client_error(&peer).await
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
    }

    /// Graceful shutdown
    pub async fn shutdown(&self) -> Result<(), RelayError> {
        info!("Initiating graceful shutdown");
        self.shutdown.send(()).map_err(|e| {
            RelayError::Connection(ConnectionError::InvalidState(format!(
                "Failed to send shutdown signal: {}",
                e
            )))
        })?;

        // Allow time for active connections to close
        sleep(Duration::from_secs(5)).await;

        Ok(())
    }
}

async fn handle_client(
    mut socket: TcpStream,
    transport: Arc<RtuTransport>,
    manager: &ConnectionManager,
    peer_addr: SocketAddr,
) -> Result<(), RelayError> {
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
        let mut tcp_buf = vec![0u8; 256];

        // Read TCP request with timeout
        let n = match timeout(Duration::from_secs(60), reader.read(&mut tcp_buf)).await {
            Ok(Ok(0)) => {
                info!("Client {} disconnected", peer_addr);
                break;
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

        // Process Modbus request
        let response = match modbus
            .process_request(
                transaction_id,
                tcp_buf[6],     // Unit ID
                &tcp_buf[7..n], // PDU
            )
            .await
        {
            Ok(response) => response,
            Err(e) => {
                manager.record_request(peer_addr, false).await;
                return Err(e);
            }
        };

        debug!("Sending TCP response to {}: {:02X?}", peer_addr, &response);

        // Send TCP response with timeout
        if (timeout(Duration::from_secs(5), writer.write_all(&response)).await).is_err() {
            manager.record_request(peer_addr, false).await;
            return Err(RelayError::client(
                ClientErrorKind::Timeout,
                peer_addr,
                "Write timeout".to_string(),
            ));
        }

        manager.record_request(peer_addr, true).await;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr};

    #[tokio::test]
    async fn test_modbus_relay_shutdown() {
        let config = RelayConfig::default();
        let relay = ModbusRelay::new(config).unwrap();

        assert!(relay.shutdown().await.is_ok());
    }
}
