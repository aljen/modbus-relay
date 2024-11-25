use std::time::{Duration, Instant};

#[cfg(any(target_os = "linux", target_os = "macos"))]
use std::os::unix::io::AsRawFd;

#[cfg(feature = "rts")]
use libc::{TIOCMGET, TIOCMSET, TIOCM_RTS};
#[cfg(any(target_os = "linux", target_os = "macos"))]
use serialport::TTYPort;

use serialport::SerialPort;
use tokio::sync::Mutex;
use tracing::{info, trace};

use crate::{RelayConfig, TransportError};

#[cfg(feature = "rts")]
use crate::RtsType;

pub struct RtuTransport {
    port: Mutex<Box<dyn SerialPort>>,
    transaction_timeout: Duration,
    trace_frames: bool,

    #[cfg(feature = "rts")]
    rts_delay_us: u64,
    #[cfg(feature = "rts")]
    rts_type: RtsType,
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    raw_fd: i32,

    rtu_flush_after_write: bool,
}

impl RtuTransport {
    pub fn new(config: &RelayConfig) -> Result<Self, TransportError> {
        info!("Opening serial port {}", config.serial_port_info());

        // Explicite otwieramy jako TTYPort na Unixie
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        let tty_port: TTYPort = serialport::new(&config.rtu_device, config.rtu_baud_rate)
            .data_bits(config.data_bits.into())
            .parity(config.parity.into())
            .stop_bits(config.stop_bits.into())
            .timeout(config.serial_timeout)
            .flow_control(serialport::FlowControl::None)
            .open_native()?;

        #[cfg(any(target_os = "linux", target_os = "macos"))]
        let raw_fd = tty_port.as_raw_fd();

        #[cfg(any(target_os = "linux", target_os = "macos"))]
        let port: Box<dyn SerialPort> = Box::new(tty_port);

        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        let port = serialport::new(&config.rtu_device, config.rtu_baud_rate)
            .data_bits(config.data_bits.into())
            .parity(config.parity.into())
            .stop_bits(config.stop_bits.into())
            .timeout(config.serial_timeout)
            .flow_control(serialport::FlowControl::None)
            .open()?;

        Ok(Self {
            port: Mutex::new(port),
            transaction_timeout: config.transaction_timeout,
            trace_frames: config.trace_frames,
            #[cfg(feature = "rts")]
            rts_delay_us: config.rtu_rts_delay_us,
            #[cfg(feature = "rts")]
            rts_type: config.rtu_rts_type,
            #[cfg(any(target_os = "linux", target_os = "macos"))]
            raw_fd,

            rtu_flush_after_write: config.rtu_flush_after_write,
        })
    }

    #[cfg(feature = "rts")]
    fn set_rts(&self, on: bool) -> Result<(), TransportError> {
        // Get raw fd from the port

        unsafe {
            let mut flags = 0i32;

            // Get current flags
            if libc::ioctl(self.raw_fd, TIOCMGET, &mut flags) < 0 {
                return Err(TransportError::Io {
                    operation: IoOperation::Control,
                    details: "Failed to get RTS flags".to_string(),
                    source: std::io::Error::last_os_error(),
                });
            }

            // Modify RTS flag
            if on {
                flags |= TIOCM_RTS; // Set RTS HIGH
            } else {
                flags &= !TIOCM_RTS; // Set RTS LOW
            }

            // Set new flags
            if libc::ioctl(self.raw_fd, TIOCMSET, &flags) < 0 {
                return Err(TransportError::Io {
                    operation: IoOperation::Control,
                    details: "Failed to set RTS flags".to_string(),
                    source: std::io::Error::last_os_error(),
                });
            }

            info!("RTS set to {}", if on { "HIGH" } else { "LOW" });
        }

        Ok(())
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    fn tc_flush(&self) -> Result<(), TransportError> {
        use crate::IoOperation;

        unsafe {
            if libc::tcflush(self.raw_fd, libc::TCIOFLUSH) != 0 {
                return Err(TransportError::Io {
                    operation: IoOperation::Flush,
                    details: "Failed to flush the serial port".to_string(),
                    source: std::io::Error::last_os_error(),
                });
            }
        }
        Ok(())
    }

    fn guess_response_size(function: u8, quantity: u16) -> usize {
        match function {
            0x01 | 0x02 => {
                // Read Coils/Discrete Inputs
                let data_bytes = (quantity as usize + 7) / 8; // Zaokrąglenie w górę
                1 + 1 + 1 + data_bytes + 2 // Unit + Func + ByteCount + Data + CRC
            }
            0x03 | 0x04 => {
                // Read Holding/Input Registers
                1 + 1 + 1 + (quantity as usize * 2) + 2 // Unit + Func + ByteCount + Data + CRC
            }
            0x05 | 0x06 => {
                // Write Single Coil/Register
                8 // Unit + Func + Addr(2) + Value(2) + CRC(2)
            }
            0x0F | 0x10 => {
                // Write Multiple Coils/Registers
                8 // Unit + Func + Addr(2) + Quantity(2) + CRC(2)
            }
            _ => 256, // Bezpieczna wartość dla nieznanych funkcji
        }
    }

    pub async fn transaction(
        &self,
        request: &[u8],
        response: &mut [u8],
    ) -> Result<usize, TransportError> {
        if self.trace_frames {
            info!("TX: {} bytes: {:02X?}", request.len(), request);
        }

        let function = request[1];
        let quantity = if function == 0x03 || function == 0x04 {
            u16::from_be_bytes([request[4], request[5]])
        } else {
            1
        };

        let expected_size = Self::guess_response_size(function, quantity);
        info!("Expected response size: {} bytes", expected_size);

        let transaction_start = Instant::now();

        tokio::time::timeout(self.transaction_timeout, async {
            let mut port = self.port.lock().await;
            let mut total_bytes = 0;

            #[cfg(feature = "rts")]
            {
                info!("RTS -> TX mode");
                self.set_rts(self.rts_type.to_signal_level(true))?;

                if self.rts_delay_us > 0 {
                    info!("RTS -> TX mode [waiting]");
                    tokio::time::sleep(Duration::from_micros(self.rts_delay_us)).await;
                }
            }

            // Write request
            info!("Writing request");
            port.write_all(request).map_err(TransportError::from)?;
            port.flush().map_err(TransportError::from)?;

            #[cfg(feature = "rts")]
            {
                info!("RTS -> RX mode");
                self.set_rts(self.rts_type.to_signal_level(false))?;
            }

            if self.rtu_flush_after_write {
                info!("RTS -> TX mode [flushing]");
                self.tc_flush()?;
            }

            #[cfg(feature = "rts")]
            if self.rts_delay_us > 0 {
                info!("RTS -> RX mode [waiting]");
                tokio::time::sleep(Duration::from_micros(self.rts_delay_us)).await;
            }

            // Read response
            trace!("Reading response (expecting {} bytes)", expected_size);
            let mut last_read_time = tokio::time::Instant::now();
            let inter_byte_timeout = Duration::from_millis(100);
            let mut consecutive_timeouts = 0;
            const MAX_TIMEOUTS: u8 = 3;

            while total_bytes < expected_size {
                match port.read(&mut response[total_bytes..]) {
                    Ok(0) => {
                        trace!("Zero bytes read");
                        if total_bytes > 0 {
                            let elapsed = last_read_time.elapsed();
                            if elapsed >= inter_byte_timeout {
                                trace!("Inter-byte timeout reached with {} bytes", total_bytes);
                                break;
                            }
                        }
                        tokio::task::yield_now().await;
                    }
                    Ok(n) => {
                        trace!(
                            "Read {} bytes: {:02X?}",
                            n,
                            &response[total_bytes..total_bytes + n]
                        );
                        total_bytes += n;
                        last_read_time = tokio::time::Instant::now();
                        consecutive_timeouts = 0;

                        if total_bytes >= expected_size {
                            trace!("Received complete response");
                            break;
                        }
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::TimedOut => {
                        trace!("Read timeout");
                        if total_bytes > 0 {
                            let elapsed = last_read_time.elapsed();
                            if elapsed >= inter_byte_timeout {
                                trace!("Inter-byte timeout reached after timeout");
                                break;
                            }
                        }
                        consecutive_timeouts += 1;
                        if consecutive_timeouts >= MAX_TIMEOUTS {
                            if total_bytes == 0 {
                                return Err(TransportError::NoResponse {
                                    attempts: consecutive_timeouts,
                                    elapsed: transaction_start.elapsed(),
                                });
                            }
                            trace!("Max timeouts reached with {} bytes", total_bytes);
                            break;
                        }
                        tokio::task::yield_now().await;
                    }
                    Err(e) => return Err(TransportError::from(e)),
                }
            }

            if total_bytes == 0 {
                info!("No response received");
                return Err(TransportError::NoResponse {
                    attempts: consecutive_timeouts,
                    elapsed: transaction_start.elapsed(),
                });
            }

            if self.trace_frames && total_bytes > 0 {
                info!(
                    "RX: {} bytes: {:02X?}",
                    total_bytes,
                    &response[..total_bytes],
                );
            }

            Ok(total_bytes)
        })
        .await
        .map_err(|elapsed| TransportError::Timeout {
            elapsed: transaction_start.elapsed(),
            limit: self.transaction_timeout,
            source: elapsed,
        })?
    }
}
