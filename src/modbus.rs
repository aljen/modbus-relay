use std::sync::Arc;

use tracing::{debug, trace};

use crate::{FrameErrorKind, RelayError, RtuTransport, errors::FrameError};

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
            let data_bytes = (quantity as usize).div_ceil(8); // Round up to the nearest whole byte
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

/// Extracts a 16-bit unsigned integer from a Modbus RTU request frame starting at the specified index.
///
/// This function attempts to retrieve two consecutive bytes from the provided request slice,
/// starting at the given index, and converts them into a `u16` value using big-endian byte order.
/// If the request slice is too short to contain the required bytes, it returns a `RelayError`
/// indicating an invalid frame format.
///
/// # Arguments
///
/// * `request` - A slice of bytes representing the Modbus RTU request frame.
/// * `start` - The starting index within the request slice from which to extract the `u16` value.
///
/// # Returns
///
/// A `Result` containing the extracted `u16` value if successful, or a `RelayError` if the request
/// slice is too short.
///
/// # Errors
///
/// Returns a `RelayError` with `FrameErrorKind::InvalidFormat` if the request slice does not contain
/// enough bytes to extract a `u16` value starting at the specified index.
fn get_u16_from_request(request: &[u8], start: usize) -> Result<u16, RelayError> {
    request
        .get(start..start + 2)
        .map(|bytes| u16::from_be_bytes([bytes[0], bytes[1]]))
        .ok_or_else(|| {
            RelayError::frame(
                FrameErrorKind::InvalidFormat,
                "Request too short for register quantity".to_string(),
                Some(request.to_vec()),
            )
        })
}

/// Extracts the quantity of coils or registers from a Modbus RTU request frame based on the function code.
///
/// This function determines the quantity of coils or registers involved in a Modbus RTU request
/// by examining the function code and extracting the appropriate bytes from the request frame.
/// For read functions (0x01 to 0x04) and write multiple functions (0x0F, 0x10), it extracts a 16-bit
/// unsigned integer from bytes 4 and 5 of the request frame. For write single functions (0x05, 0x06),
/// it returns a fixed quantity of 1. For other function codes, it defaults to a quantity of 1.
///
/// # Arguments
///
/// * `function_code` - The Modbus function code.
/// * `request` - A slice of bytes representing the Modbus RTU request frame.
///
/// # Returns
///
/// A `Result` containing the extracted quantity as a `u16` value if successful, or a `RelayError` if the request
/// slice is too short or the function code is invalid.
///
/// # Errors
///
/// Returns a `RelayError` with `FrameErrorKind::InvalidFormat` if the request slice does not contain
/// enough bytes to extract the quantity for the specified function code.
pub fn get_quantity(function_code: u8, request: &[u8]) -> Result<u16, RelayError> {
    match function_code {
        // For read functions (0x01 to 0x04) and write multiple functions (0x0F, 0x10),
        // extract the quantity from bytes 4 and 5 of the request frame.
        0x01..=0x04 | 0x0F | 0x10 => get_u16_from_request(request, 4),

        // For write single functions (0x05, 0x06), the quantity is always 1.
        0x05 | 0x06 => Ok(1),

        // For other function codes, default the quantity to 1.
        _ => Ok(1),
    }
}

pub struct ModbusProcessor {
    transport: Arc<RtuTransport>,
}

impl ModbusProcessor {
    pub fn new(transport: Arc<RtuTransport>) -> Self {
        Self { transport }
    }

    /// Processes a Modbus TCP request by converting it to Modbus RTU, sending it over the transport,
    /// and then converting the RTU response back to Modbus TCP format.
    ///
    /// # Arguments
    ///
    /// * `transaction_id` - The Modbus TCP transaction ID.
    /// * `unit_id` - The Modbus unit ID (slave address).
    /// * `pdu` - The Protocol Data Unit from the Modbus TCP request.
    ///
    /// # Returns
    ///
    /// A `Result` containing the Modbus TCP response as a vector of bytes, or a `RelayError`.
    pub async fn process_request(
        &self,
        transaction_id: [u8; 2],
        unit_id: u8,
        pdu: &[u8],
        trace_frames: bool,
    ) -> Result<Vec<u8>, RelayError> {
        // Build RTU request frame: [Unit ID][PDU][CRC16]
        let mut rtu_request = Vec::with_capacity(1 + pdu.len() + 2); // Unit ID + PDU + CRC16
        rtu_request.push(unit_id);
        rtu_request.extend_from_slice(pdu);

        // Calculate CRC16 checksum and append to the request
        let crc = calc_crc16(&rtu_request);
        rtu_request.extend_from_slice(&crc.to_le_bytes()); // Append CRC16 in little-endian

        if trace_frames {
            trace!(
                "Sending RTU request: unit_id=0x{:02X}, function=0x{:02X}, data={:02X?}, crc=0x{:04X}",
                unit_id,
                pdu.first().copied().unwrap_or(0),
                &pdu[1..],
                crc
            );
        }

        // Estimate the expected RTU response size
        let function_code = pdu.first().copied().unwrap_or(0);
        let quantity = get_quantity(function_code, &rtu_request)?;

        let expected_response_size = guess_response_size(function_code, quantity);

        // Allocate buffer for RTU response
        let mut rtu_response = vec![0u8; expected_response_size];

        // Execute RTU transaction
        let rtu_len = match self
            .transport
            .transaction(&rtu_request, &mut rtu_response)
            .await
        {
            Ok(len) => {
                if len < 5 {
                    // Minimum RTU response size: Unit ID(1) + Function(1) + Data(1) + CRC(2)
                    return Err(RelayError::frame(
                        FrameErrorKind::TooShort,
                        format!("RTU response too short: {} bytes", len),
                        Some(rtu_response[..len].to_vec()),
                    ));
                }
                len
            }
            Err(e) => {
                debug!("Transport transaction error: {:?}", e);

                // Prepare Modbus exception response with exception code 0x0B (Gateway Path Unavailable)
                let exception_code = 0x0B;
                let mut exception_response = Vec::with_capacity(9);
                exception_response.extend_from_slice(&transaction_id);
                exception_response.extend_from_slice(&[0x00, 0x00]); // Protocol ID
                exception_response.extend_from_slice(&[0x00, 0x03]); // Length (Unit ID + Function + Exception Code)
                exception_response.push(unit_id);
                exception_response.push(function_code | 0x80); // Exception function code
                exception_response.push(exception_code);

                return Ok(exception_response);
            }
        };

        // Truncate the buffer to the actual response length
        rtu_response.truncate(rtu_len);

        // Verify the CRC16 checksum of the RTU response
        let expected_crc = calc_crc16(&rtu_response[..rtu_len - 2]);
        let received_crc =
            u16::from_le_bytes([rtu_response[rtu_len - 2], rtu_response[rtu_len - 1]]);
        if expected_crc != received_crc {
            return Err(RelayError::Frame(FrameError::Crc {
                calculated: expected_crc,
                received: received_crc,
                frame_hex: hex::encode(&rtu_response[..rtu_len - 2]),
            }));
        }

        // Remove CRC from RTU response
        rtu_response.truncate(rtu_len - 2);

        // Verify that the unit ID in the response matches
        if rtu_response[0] != unit_id {
            return Err(RelayError::frame(
                FrameErrorKind::InvalidUnitId,
                format!(
                    "Unexpected unit ID in RTU response: expected=0x{:02X}, received=0x{:02X}",
                    unit_id, rtu_response[0]
                ),
                Some(rtu_response.clone()),
            ));
        }

        // Convert RTU response to Modbus TCP response
        let tcp_length = rtu_response.len() as u16; // Length of Unit ID + PDU
        let mut tcp_response = Vec::with_capacity(7 + rtu_response.len()); // MBAP Header(7) + PDU
        tcp_response.extend_from_slice(&transaction_id); // Transaction ID
        tcp_response.extend_from_slice(&[0x00, 0x00]); // Protocol ID
        tcp_response.extend_from_slice(&tcp_length.to_be_bytes()); // Length field
        tcp_response.extend_from_slice(&rtu_response); // Unit ID + PDU

        Ok(tcp_response)
    }
}
