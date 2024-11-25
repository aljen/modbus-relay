use hex;
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
    errors::{ClientErrorKind, FrameErrorKind, ProtocolErrorKind, RelayError},
    relay_config::RelayConfig,
    rtu_transport::RtuTransport,
};

pub struct ModbusRelay {
    transport: Arc<RtuTransport>,
    config: RelayConfig,
    connection_manager: Arc<ConnectionManager>,
    shutdown: broadcast::Sender<()>,
}

impl ModbusRelay {
    pub fn new(config: RelayConfig) -> Result<Self, RelayError> {
        let transport = RtuTransport::new(&config).map_err(RelayError::Transport)?;
        let conn_config = ConnectionConfig::default(); // TODO: Add to RelayConfig

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
        let listener = TcpListener::bind(&addr).await?;
        info!("Listening on {}", addr);

        // Start a task to clean up idle connections
        let manager = Arc::clone(&self.connection_manager);
        let mut shutdown_rx = self.shutdown.subscribe();

        self.spawn_task("cleanup", async move {
            loop {
                tokio::select! {
                    _ = sleep(Duration::from_secs(60)) => {
                        manager.cleanup_idle_connections().await;
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
                        let stats = manager.get_stats().await;
                        info!("Connection stats: {:?}", stats);
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
    pub async fn shutdown(&self) {
        info!("Initiating graceful shutdown");
        let _ = self.shutdown.send(());

        // Allow time for active connections to close
        sleep(Duration::from_secs(5)).await;
    }
}

fn calc_crc16(frame: &[u8], data_length: u8) -> u16 {
    let mut crc: u16 = 0xffff;
    for i in frame.iter().take(data_length as usize) {
        crc ^= u16::from(*i);
        for _ in (0..8).rev() {
            if (crc & 0x0001) == 0 {
                crc >>= 1;
            } else {
                crc >>= 1;
                crc ^= 0xA001;
            }
        }
    }
    crc
}

async fn handle_client(
    mut socket: TcpStream,
    transport: Arc<RtuTransport>,
    manager: &ConnectionManager,
    peer_addr: SocketAddr,
) -> Result<(), RelayError> {
    socket.set_nodelay(true)?;

    let addr = socket.peer_addr()?;
    info!("New client connected from {}", addr);

    let (mut reader, mut writer) = socket.split();

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
                return Err(RelayError::client(
                    ClientErrorKind::ConnectionLost,
                    peer_addr,
                    e.to_string(),
                ));
            }
            Err(_) => {
                manager.record_request(peer_addr, false).await;
                return Err(RelayError::client(
                    ClientErrorKind::Timeout,
                    peer_addr,
                    "Read timeout".to_string(),
                ));
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

        // Convert TCP to RTU
        let mut rtu_request = Vec::with_capacity(256);
        rtu_request.push(tcp_buf[6]); // Unit ID
        rtu_request.extend_from_slice(&tcp_buf[7..n]); // Function code and data

        let crc = calc_crc16(&rtu_request, rtu_request.len() as u8);
        rtu_request.extend_from_slice(&crc.to_le_bytes());

        debug!(
            "Sending RTU request to device: data={:02X?}, crc={:04X}",
            &rtu_request[..rtu_request.len() - 2],
            crc
        );

        // Execute RTU transaction
        let mut rtu_buf = vec![0u8; 256];
        let rtu_len = match transport.transaction(&rtu_request, &mut rtu_buf).await {
            Ok(len) => {
                if len < 3 {
                    manager.record_request(peer_addr, false).await;
                    return Err(RelayError::frame(
                        FrameErrorKind::TooShort,
                        format!("RTU response too short: {} bytes", len),
                        Some(rtu_buf[..len].to_vec()),
                    ));
                }
                len
            }
            Err(e) => {
                manager.record_request(peer_addr, false).await;

                // Prepare Modbus exception response
                let mut exception_response = Vec::new();
                exception_response.extend_from_slice(&transaction_id);
                exception_response.extend_from_slice(&[0x00, 0x00]);
                exception_response.extend_from_slice(&[0x00, 0x03]);
                exception_response.push(tcp_buf[6]);
                exception_response.push(tcp_buf[7] | 0x80);
                exception_response.push(0x0B);

                if let Err(e) = writer.write_all(&exception_response).await {
                    return Err(RelayError::client(
                        ClientErrorKind::ConnectionLost,
                        peer_addr,
                        format!("Failed to send exception response: {}", e),
                    ));
                }

                return Err(RelayError::Transport(e));
            }
        };

        // Verify RTU CRC
        let calculated_crc = calc_crc16(&rtu_buf[..rtu_len - 2], (rtu_len - 2) as u8);
        let received_crc = u16::from_le_bytes([rtu_buf[rtu_len - 2], rtu_buf[rtu_len - 1]]);

        if calculated_crc != received_crc {
            manager.record_request(peer_addr, false).await;
            return Err(RelayError::InvalidCrc {
                calculated: calculated_crc,
                received: received_crc,
                frame_hex: hex::encode(&rtu_buf[..rtu_len - 2]),
            });
        }

        // Convert RTU to TCP
        let mut tcp_response = Vec::with_capacity(256);
        tcp_response.extend_from_slice(&transaction_id);
        tcp_response.extend_from_slice(&[0x00, 0x00]);

        let tcp_length = (rtu_len - 2) as u16;
        tcp_response.extend_from_slice(&tcp_length.to_be_bytes());
        tcp_response.extend_from_slice(&rtu_buf[..rtu_len - 2]);

        debug!(
            "Sending TCP response to {}: {:02X?}",
            peer_addr, &tcp_response
        );

        // Send TCP response with timeout
        if let Err(_) = timeout(Duration::from_secs(5), writer.write_all(&tcp_response)).await {
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
