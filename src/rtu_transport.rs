use std::time::{Duration, Instant};

#[cfg(any(target_os = "linux", target_os = "macos"))]
use std::os::unix::io::AsRawFd;

#[cfg(any(target_os = "linux", target_os = "macos"))]
use libc::{TIOCMGET, TIOCMSET, TIOCM_RTS};

#[cfg(any(target_os = "linux", target_os = "macos"))]
use serialport::TTYPort;

use serialport::SerialPort;
use tokio::sync::Mutex;
use tracing::{info, trace};

use crate::{RtsError, RtsType};

use crate::{FrameErrorKind, IoOperation, RelayError, RtuConfig, TransportError};

pub struct RtuTransport {
    port: Mutex<Box<dyn SerialPort>>,
    config: RtuConfig,
    trace_frames: bool,

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    raw_fd: i32,
}

impl RtuTransport {
    pub fn new(config: &RtuConfig, trace_frames: bool) -> Result<Self, TransportError> {
        info!("Opening serial port {}", config.serial_port_info());

        // Explicitly open as TTYPort on Unix
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        let tty_port: TTYPort = serialport::new(&config.device, config.baud_rate)
            .data_bits(config.data_bits.into())
            .parity(config.parity.into())
            .stop_bits(config.stop_bits.into())
            .timeout(config.serial_timeout)
            .flow_control(serialport::FlowControl::None)
            .open_native()
            .map_err(|e| TransportError::Io {
                operation: IoOperation::Configure,
                details: format!("serial port {}", config.device),
                source: std::io::Error::new(std::io::ErrorKind::Other, e.description),
            })?;

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
            .open()
            .map_err(|e| TransportError::Io {
                operation: IoOperation::Configure,
                details: format!("serial port {}", config.rtu_device),
                source: std::io::Error::new(std::io::ErrorKind::Other, e.description),
            })?;

        Ok(Self {
            port: Mutex::new(port),
            config: config.clone(),
            trace_frames,
            #[cfg(any(target_os = "linux", target_os = "macos"))]
            raw_fd,
        })
    }

    pub async fn close(&self) -> Result<(), TransportError> {
        let port = self.port.lock().await;
        port.clear(serialport::ClearBuffer::All)
            .map_err(|e| TransportError::Io {
                operation: IoOperation::Flush,
                details: "Failed to clear buffers".to_string(),
                source: std::io::Error::new(std::io::ErrorKind::Other, e.description),
            })?;

        #[cfg(unix)]
        unsafe {
            if libc::close(self.raw_fd) != 0 {
                return Err(TransportError::Io {
                    operation: IoOperation::Control,
                    details: "Failed to close serial port".to_string(),
                    source: std::io::Error::last_os_error(),
                });
            }
        }

        Ok(())
    }

    fn set_rts(&self, on: bool, trace_frames: bool) -> Result<(), TransportError> {
        let rts_span = tracing::info_span!(
            "rts_control",
            signal = if on { "HIGH" } else { "LOW" },
            delay_us = self.config.rts_delay_us,
        );
        let _enter = rts_span.enter();

        unsafe {
            let mut flags = 0i32;

            // Get current flags
            if libc::ioctl(self.raw_fd, TIOCMGET, &mut flags) < 0 {
                let err = std::io::Error::last_os_error();
                return Err(TransportError::Rts(RtsError::signal(format!(
                    "Failed to get RTS flags: {} (errno: {})",
                    err,
                    err.raw_os_error().unwrap_or(-1)
                ))));
            }

            // Modify RTS flag
            if on {
                flags |= TIOCM_RTS; // Set RTS HIGH
            } else {
                flags &= !TIOCM_RTS; // Set RTS LOW
            }

            // Set new flags
            if libc::ioctl(self.raw_fd, TIOCMSET, &flags) < 0 {
                let err = std::io::Error::last_os_error();
                return Err(TransportError::Rts(RtsError::signal(format!(
                    "Failed to set RTS flags: {} (errno: {})",
                    err,
                    err.raw_os_error().unwrap_or(-1)
                ))));
            }

            if trace_frames {
                trace!("RTS set to {}", if on { "HIGH" } else { "LOW" });
            }
        }

        Ok(())
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    fn tc_flush(&self) -> Result<(), TransportError> {
        unsafe {
            if libc::tcflush(self.raw_fd, libc::TCIOFLUSH) != 0 {
                return Err(TransportError::Io {
                    operation: IoOperation::Flush,
                    details: format!(
                        "Failed to flush serial port: {}",
                        std::io::Error::last_os_error()
                    ),
                    source: std::io::Error::last_os_error(),
                });
            }
        }
        Ok(())
    }

    pub async fn transaction(
        &self,
        request: &[u8],
        response: &mut [u8],
    ) -> Result<usize, RelayError> {
        if request.len() > self.config.max_frame_size as usize {
            return Err(RelayError::frame(
                FrameErrorKind::TooLong,
                format!("Request frame too long: {} bytes", request.len()),
                Some(request.to_vec()),
            ));
        }

        let expected_size = response.len();

        if self.trace_frames {
            trace!("TX: {} bytes: {:02X?}", request.len(), request);
            trace!("Expected response size: {} bytes", expected_size);
        }

        let transaction_start = Instant::now();

        let result = tokio::time::timeout(self.config.transaction_timeout, async {
            let mut port = self.port.lock().await;

            if self.config.rts_type != RtsType::None {
                if self.trace_frames {
                    trace!("RTS -> TX mode");
                }

                self.set_rts(
                    self.config.rts_type.to_signal_level(true),
                    self.trace_frames,
                )?;

                if self.config.rts_delay_us > 0 {
                    if self.trace_frames {
                        trace!("RTS -> TX mode [waiting]");
                    }
                    tokio::time::sleep(Duration::from_micros(self.config.rts_delay_us)).await;
                }
            }

            // Write request
            if self.trace_frames {
                trace!("Writing request");
            }
            port.write_all(request).map_err(|e| TransportError::Io {
                operation: IoOperation::Write,
                details: "Failed to write request".to_string(),
                source: e,
            })?;

            port.flush().map_err(|e| TransportError::Io {
                operation: IoOperation::Flush,
                details: "Failed to flush write buffer".to_string(),
                source: e,
            })?;

            if self.config.rts_type != RtsType::None {
                if self.trace_frames {
                    trace!("RTS -> RX mode");
                }

                self.set_rts(
                    self.config.rts_type.to_signal_level(false),
                    self.trace_frames,
                )?;
            }

            if self.config.flush_after_write {
                if self.trace_frames {
                    trace!("RTS -> TX mode [flushing]");
                }
                self.tc_flush()?;
            }

            if self.config.rts_type != RtsType::None && self.config.rts_delay_us > 0 {
                if self.trace_frames {
                    trace!("RTS -> RX mode [waiting]");
                }
                tokio::time::sleep(Duration::from_micros(self.config.rts_delay_us)).await;
            }

            // Read response
            if self.trace_frames {
                trace!("Reading response (expecting {} bytes)", expected_size);
            }

            const MAX_TIMEOUTS: u8 = 3;
            let mut total_bytes = 0;
            let mut consecutive_timeouts = 0;
            let inter_byte_timeout = Duration::from_millis(100);
            let mut last_read_time = tokio::time::Instant::now();

            while total_bytes < expected_size {
                match port.read(&mut response[total_bytes..]) {
                    Ok(0) => {
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
                        if self.trace_frames {
                            trace!(
                                "Read {} bytes: {:02X?}",
                                n,
                                &response[total_bytes..total_bytes + n]
                            );
                        }
                        total_bytes += n;
                        last_read_time = tokio::time::Instant::now();
                        consecutive_timeouts = 0;

                        if total_bytes >= expected_size {
                            if self.trace_frames {
                                trace!("Received complete response");
                            }
                            break;
                        }
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::TimedOut => {
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
                    Err(e) => {
                        return Err(TransportError::Io {
                            operation: IoOperation::Read,
                            details: "Failed to read response".to_string(),
                            source: e,
                        });
                    }
                }
            }

            if total_bytes == 0 {
                return Err(TransportError::NoResponse {
                    attempts: consecutive_timeouts,
                    elapsed: transaction_start.elapsed(),
                });
            }

            // Verify minimum response size
            if total_bytes < 3 {
                return Err(TransportError::Io {
                    operation: IoOperation::Read,
                    details: format!("Response too short: {} bytes", total_bytes),
                    source: std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "Response too short",
                    ),
                });
            }

            if self.trace_frames {
                trace!(
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
            limit: self.config.transaction_timeout,
            source: elapsed,
        })?;

        Ok(result?)
    }
}
