use std::sync::Arc;

use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};
use tracing::{error, info, warn};

use crate::{relay_config::RelayConfig, rtu_transport::RtuTransport, RelayError};

pub struct ModbusRelay {
    transport: Arc<RtuTransport>,
    config: RelayConfig,
}

impl ModbusRelay {
    pub fn new(config: RelayConfig) -> Result<Self, RelayError> {
        let transport = RtuTransport::new(&config).map_err(RelayError::Transport)?;

        Ok(Self {
            transport: Arc::new(transport),
            config,
        })
    }

    pub async fn run(&self) -> Result<(), RelayError> {
        let addr = format!(
            "{}:{}",
            self.config.tcp_bind_addr, self.config.tcp_bind_port
        );
        let listener = TcpListener::bind(&addr).await?;
        info!("Listening on {}", addr);

        loop {
            let (socket, peer) = listener.accept().await?;
            info!("New connection from {}", peer);

            let transport = Arc::clone(&self.transport);
            tokio::spawn(async move {
                if let Err(e) = handle_client(socket, transport).await {
                    error!("Client error: {}", e);
                }
            });
        }
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
) -> Result<(), RelayError> {
    let addr = socket.peer_addr()?;
    info!("New client connected from {}", addr);

    let (mut reader, mut writer) = socket.split();

    loop {
        let mut tcp_buf = vec![0u8; 256];

        // Read TCP request
        let n = match reader.read(&mut tcp_buf).await {
            Ok(0) => {
                info!("Client {} disconnected", addr);
                break;
            }
            Ok(n) => {
                if n < 7 {
                    // MBAP header minimum length
                    warn!("Received too short frame from {}: {} bytes", addr, n);
                    continue;
                }
                n
            }
            Err(e) => {
                error!("Error reading from client {}: {}", addr, e);
                return Err(e.into());
            }
        };

        info!("Received TCP frame from {}: {:02X?}", addr, &tcp_buf[..n]);

        // Validate MBAP header
        let transaction_id = [tcp_buf[0], tcp_buf[1]];
        let protocol_id = u16::from_be_bytes([tcp_buf[2], tcp_buf[3]]);
        if protocol_id != 0 {
            error!("Invalid protocol ID from {}: {}", addr, protocol_id);
            continue; // Skip this frame but keep connection
        }

        let length = u16::from_be_bytes([tcp_buf[4], tcp_buf[5]]) as usize;
        if length > 249 {
            // 256 - 7 (MBAP header)
            error!("Frame too long from {}: {}", addr, length);
            continue;
        }

        if length + 6 != n {
            warn!(
                "Invalid frame length from {}, expected {}, got {}",
                addr,
                length + 6,
                n
            );
            continue;
        }

        // Convert TCP to RTU
        let mut rtu_request = Vec::with_capacity(256);
        rtu_request.push(tcp_buf[6]); // Unit ID
        rtu_request.extend_from_slice(&tcp_buf[7..n]); // Function code and data

        let crc = calc_crc16(&rtu_request, rtu_request.len() as u8);
        rtu_request.extend_from_slice(&crc.to_le_bytes());

        info!(
            "Sending RTU request to device: data={:02X?}, crc={:04X}",
            &rtu_request[..rtu_request.len() - 2],
            crc
        );

        // Execute RTU transaction
        let mut rtu_buf = vec![0u8; 256];
        let rtu_len = match transport.transaction(&rtu_request, &mut rtu_buf).await {
            Ok(len) => {
                if len < 3 {
                    // Minimum valid RTU response (1 byte function code + 2 bytes CRC)
                    error!("RTU response too short from device: {} bytes", len);
                    continue;
                }
                len
            }
            Err(e) => {
                error!("RTU transaction error for {}: {}", addr, e);
                // Prepare Modbus exception response
                let mut exception_response = Vec::new();
                exception_response.extend_from_slice(&transaction_id); // Original transaction ID
                exception_response.extend_from_slice(&[0x00, 0x00]); // Protocol ID
                exception_response.extend_from_slice(&[0x00, 0x03]); // Length
                exception_response.push(tcp_buf[6]); // Unit ID
                exception_response.push(tcp_buf[7] | 0x80); // Function code with error bit set
                exception_response.push(0x0B); // Gateway target device failed to respond

                if let Err(e) = writer.write_all(&exception_response).await {
                    error!("Failed to send exception response to {}: {}", addr, e);
                    return Err(e.into());
                }
                continue;
            }
        };

        info!(
            "Received RTU response from device: {:02X?}",
            &rtu_buf[..rtu_len]
        );

        // Verify RTU CRC
        let calculated_crc = calc_crc16(&rtu_buf[..rtu_len - 2], (rtu_len - 2) as u8);
        let received_crc = u16::from_le_bytes([rtu_buf[rtu_len - 2], rtu_buf[rtu_len - 1]]);
        if calculated_crc != received_crc {
            error!(
              "CRC error in RTU response from device: data={:02X?}, calculated={:04X}, received={:04X}",
              &rtu_buf[..rtu_len - 2], calculated_crc, received_crc
          );
            continue;
        }

        info!("CRC verification passed: {:04X}", calculated_crc);

        // Convert RTU to TCP
        let mut tcp_response = Vec::with_capacity(256);

        // MBAP header
        tcp_response.extend_from_slice(&transaction_id); // Original transaction ID
        tcp_response.extend_from_slice(&[0x00, 0x00]); // Protocol ID

        let tcp_length = (rtu_len - 2) as u16; // Remove CRC bytes from length
        tcp_response.extend_from_slice(&tcp_length.to_be_bytes());

        // Copy Unit ID and PDU, excluding CRC
        tcp_response.extend_from_slice(&rtu_buf[..rtu_len - 2]);

        info!("Sending TCP response to {}: {:02X?}", addr, &tcp_response);

        // Send TCP response
        if let Err(e) = writer.write_all(&tcp_response).await {
            error!("Error writing response to {}: {}", addr, e);
            return Err(e.into());
        }
    }

    Ok(())
}
