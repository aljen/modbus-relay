use std::sync::Arc;

use tracing::debug;

use crate::{errors::FrameError, FrameErrorKind, RelayError, RtuTransport};

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

pub struct ModbusProcessor {
    transport: Arc<RtuTransport>,
}

impl ModbusProcessor {
    pub fn new(transport: Arc<RtuTransport>) -> Self {
        Self { transport }
    }

    pub async fn process_request(
        &self,
        transaction_id: [u8; 2],
        unit_id: u8,
        pdu: &[u8],
    ) -> Result<Vec<u8>, RelayError> {
        // Convert TCP to RTU
        let mut rtu_request = Vec::with_capacity(256);
        rtu_request.push(unit_id);
        rtu_request.extend_from_slice(pdu);

        let crc = calc_crc16(&rtu_request, rtu_request.len() as u8);
        rtu_request.extend_from_slice(&crc.to_le_bytes());

        debug!(
            "Sending RTU request to device: data={:02X?}, crc={:04X}",
            &rtu_request[..rtu_request.len() - 2],
            crc
        );

        // Execute RTU transaction
        let mut rtu_buf = vec![0u8; 256];
        let rtu_len = match self.transport.transaction(&rtu_request, &mut rtu_buf).await {
            Ok(len) => {
                if len < 3 {
                    return Err(RelayError::frame(
                        FrameErrorKind::TooShort,
                        format!("RTU response too short: {} bytes", len),
                        Some(rtu_buf[..len].to_vec()),
                    ));
                }
                len
            }
            Err(_) => {
                // Prepare Modbus exception response
                let mut exception_response = Vec::new();
                exception_response.extend_from_slice(&transaction_id);
                exception_response.extend_from_slice(&[0x00, 0x00]);
                exception_response.extend_from_slice(&[0x00, 0x03]);
                exception_response.push(unit_id);
                exception_response.push(pdu[0] | 0x80);
                exception_response.push(0x0B);

                return Ok(exception_response);
            }
        };

        // Verify RTU CRC
        let calculated_crc = calc_crc16(&rtu_buf[..rtu_len - 2], (rtu_len - 2) as u8);
        let received_crc = u16::from_le_bytes([rtu_buf[rtu_len - 2], rtu_buf[rtu_len - 1]]);

        if calculated_crc != received_crc {
            return Err(RelayError::Frame(FrameError::Crc {
                calculated: calculated_crc,
                received: received_crc,
                frame_hex: hex::encode(&rtu_buf[..rtu_len - 2]),
            }));
        }

        // Convert RTU to TCP
        let mut tcp_response = Vec::with_capacity(256);
        tcp_response.extend_from_slice(&transaction_id);
        tcp_response.extend_from_slice(&[0x00, 0x00]);

        let tcp_length = (rtu_len - 2) as u16;
        tcp_response.extend_from_slice(&tcp_length.to_be_bytes());
        tcp_response.extend_from_slice(&rtu_buf[..rtu_len - 2]);

        Ok(tcp_response)
    }
}
