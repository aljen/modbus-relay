use std::sync::Arc;

use tracing::debug;

use crate::{errors::FrameError, FrameErrorKind, RelayError, RtuTransport};

/// Calculates the CRC16 checksum for Modbus RTU communication using a lookup table for high performance.
///
/// This function computes the CRC16-Modbus checksum for the provided data frame.
/// It uses a precomputed lookup table to optimize performance by eliminating
/// bitwise calculations within the inner loop.
///
/// # Arguments
///
/// * `data` - A slice of bytes representing the data frame for which the CRC is to be computed.
///
/// # Returns
///
/// The computed 16-bit CRC as a `u16` value.
///
/// # Example
///
/// ```rust
/// let frame: [u8; 6] = [0x01, 0x03, 0x00, 0x00, 0x00, 0x0A];
/// let crc = calc_crc16(&frame);
/// ```
fn calc_crc16(data: &[u8]) -> u16 {
    // Precomputed CRC16 lookup table for polynomial 0xA001 (Modbus standard)
    const CRC16_TABLE: [u16; 256] = [
        0x0000, 0xC0C1, 0xC181, 0x0140, 0xC301, 0x03C0, 0x0280, 0xC241, 0xC601, 0x06C0, 0x0780,
        0xC741, 0x0500, 0xC5C1, 0xC481, 0x0440, 0xCC01, 0x0CC0, 0x0D80, 0xCD41, 0x0F00, 0xCFC1,
        0xCE81, 0x0E40, 0x0A00, 0xCAC1, 0xCB81, 0x0B40, 0xC901, 0x09C0, 0x0880, 0xC841, 0xD801,
        0x18C0, 0x1980, 0xD941, 0x1B00, 0xDBC1, 0xDA81, 0x1A40, 0x1E00, 0xDEC1, 0xDF81, 0x1F40,
        0xDD01, 0x1DC0, 0x1C80, 0xDC41, 0x1400, 0xD4C1, 0xD581, 0x1540, 0xD701, 0x17C0, 0x1680,
        0xD641, 0xD201, 0x12C0, 0x1380, 0xD341, 0x1100, 0xD1C1, 0xD081, 0x1040, 0xF001, 0x30C0,
        0x3180, 0xF141, 0x3300, 0xF3C1, 0xF281, 0x3240, 0x3600, 0xF6C1, 0xF781, 0x3740, 0xF501,
        0x35C0, 0x3480, 0xF441, 0x3C00, 0xFCC1, 0xFD81, 0x3D40, 0xFF01, 0x3FC0, 0x3E80, 0xFE41,
        0xFA01, 0x3AC0, 0x3B80, 0xFB41, 0x3900, 0xF9C1, 0xF881, 0x3840, 0x2800, 0xE8C1, 0xE981,
        0x2940, 0xEB01, 0x2BC0, 0x2A80, 0xEA41, 0xEE01, 0x2EC0, 0x2F80, 0xEF41, 0x2D00, 0xEDC1,
        0xEC81, 0x2C40, 0xE401, 0x24C0, 0x2580, 0xE541, 0x2700, 0xE7C1, 0xE681, 0x2640, 0x2200,
        0xE2C1, 0xE381, 0x2340, 0xE101, 0x21C0, 0x2080, 0xE041, 0xA001, 0x60C0, 0x6180, 0xA141,
        0x6300, 0xA3C1, 0xA281, 0x6240, 0x6600, 0xA6C1, 0xA781, 0x6740, 0xA501, 0x65C0, 0x6480,
        0xA441, 0x6C00, 0xACC1, 0xAD81, 0x6D40, 0xAF01, 0x6FC0, 0x6E80, 0xAE41, 0xAA01, 0x6AC0,
        0x6B80, 0xAB41, 0x6900, 0xA9C1, 0xA881, 0x6840, 0x7800, 0xB8C1, 0xB981, 0x7940, 0xBB01,
        0x7BC0, 0x7A80, 0xBA41, 0xBE01, 0x7EC0, 0x7F80, 0xBF41, 0x7D00, 0xBDC1, 0xBC81, 0x7C40,
        0xB401, 0x74C0, 0x7580, 0xB541, 0x7700, 0xB7C1, 0xB681, 0x7640, 0x7200, 0xB2C1, 0xB381,
        0x7340, 0xB101, 0x71C0, 0x7080, 0xB041, 0x5000, 0x90C1, 0x9181, 0x5140, 0x9301, 0x53C0,
        0x5280, 0x9241, 0x9601, 0x56C0, 0x5780, 0x9741, 0x5500, 0x95C1, 0x9481, 0x5440, 0x9C01,
        0x5CC0, 0x5D80, 0x9D41, 0x5F00, 0x9FC1, 0x9E81, 0x5E40, 0x5A00, 0x9AC1, 0x9B81, 0x5B40,
        0x9901, 0x59C0, 0x5880, 0x9841, 0x8801, 0x48C0, 0x4980, 0x8941, 0x4B00, 0x8BC1, 0x8A81,
        0x4A40, 0x4E00, 0x8EC1, 0x8F81, 0x4F40, 0x8D01, 0x4DC0, 0x4C80, 0x8C41, 0x4400, 0x84C1,
        0x8581, 0x4540, 0x8701, 0x47C0, 0x4680, 0x8641, 0x8201, 0x42C0, 0x4380, 0x8341, 0x4100,
        0x81C1, 0x8081, 0x4040,
    ];

    let mut crc: u16 = 0xFFFF; // Initialize CRC to 0xFFFF as per Modbus standard

    for &byte in data {
        // XOR the lower byte of the CRC with the current byte and find the lookup table index
        let index = ((crc ^ byte as u16) & 0x00FF) as usize;
        // Update the CRC by shifting right and XORing with the table value
        crc = (crc >> 8) ^ CRC16_TABLE[index];
    }

    crc
}

/// Estimates the expected size of a Modbus RTU response frame based on the function code and quantity.
///
/// # Arguments
///
/// * `function` - The Modbus function code.
/// * `quantity` - The number of coils or registers involved.
///
/// # Returns
///
/// The estimated size of the response frame in bytes.
pub fn guess_response_size(function: u8, quantity: u16) -> usize {
    match function {
        0x01 | 0x02 => {
            // Read Coils / Read Discrete Inputs
            // Each coil status is one bit; calculate the number of data bytes required
            let data_bytes = ((quantity as usize) + 7) / 8; // Round up to the nearest whole byte
                                                            // Response size: Address(1) + Function(1) + Byte Count(1) + Data + CRC(2)
            1 + 1 + 1 + data_bytes + 2
        }
        0x03 | 0x04 => {
            // Read Holding Registers / Read Input Registers
            // Each register is two bytes
            let data_bytes = (quantity as usize) * 2;
            // Response size: Address(1) + Function(1) + Byte Count(1) + Data + CRC(2)
            1 + 1 + 1 + data_bytes + 2
        }
        0x05 | 0x06 => {
            // Write Single Coil / Write Single Register
            // Response size: Address(1) + Function(1) + Address(2) + Value(2) + CRC(2)
            1 + 1 + 2 + 2 + 2
        }
        0x0F | 0x10 => {
            // Write Multiple Coils / Write Multiple Registers
            // Response size: Address(1) + Function(1) + Address(2) + Quantity(2) + CRC(2)
            1 + 1 + 2 + 2 + 2
        }
        _ => {
            // Default maximum size for unknown function codes
            256
        }
    }
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

        let crc = calc_crc16(&rtu_request);
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
        let calculated_crc = calc_crc16(&rtu_buf[..rtu_len - 2]);
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
