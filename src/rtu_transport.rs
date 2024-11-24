use serialport::SerialPort;
use std::time::Duration;
use thiserror::Error;
use tokio::{sync::Mutex, time::error::Elapsed};
use tracing::error;

#[derive(Error, Debug)]
pub enum TransportError {
    #[error("Serial port error: {0}")]
    Serial(#[from] serialport::Error),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Transaction timeout")]
    Timeout(#[from] Elapsed),
}

pub struct RtuTransport {
    port: Mutex<Box<dyn SerialPort>>,
    transaction_timeout: Duration,

    #[cfg(feature = "rts")]
    rts_delay_ms: u64,
}

impl RtuTransport {
    pub fn new(
        device: &str,
        baud_rate: u32,
        transaction_timeout: Duration,
        #[cfg(feature = "rts")] rts_delay_ms: u64,
    ) -> Result<Self, TransportError> {
        let port = serialport::new(device, baud_rate)
            .data_bits(serialport::DataBits::Eight)
            .parity(serialport::Parity::None)
            .stop_bits(serialport::StopBits::One)
            .timeout(Duration::from_millis(100))
            .open()?;

        Ok(Self {
            port: Mutex::new(port),
            transaction_timeout,
            #[cfg(feature = "rts")]
            rts_delay_ms,
        })
    }

    pub async fn transaction(
        &self,
        request: &[u8],
        response: &mut [u8],
    ) -> Result<usize, TransportError> {
        tokio::time::timeout(self.transaction_timeout, async {
            let mut port = self.port.lock().await;

            #[cfg(feature = "rts")]
            {
                port.write_request_to_send(true)?;
                if self.rts_delay_ms > 0 {
                    tokio::time::sleep(Duration::from_millis(self.rts_delay_ms)).await;
                }
            }

            port.write_all(request)?;
            port.flush()?;

            #[cfg(feature = "rts")]
            {
                port.write_request_to_send(false)?;
                if self.rts_delay_ms > 0 {
                    tokio::time::sleep(Duration::from_millis(self.rts_delay_ms)).await;
                }
            }

            let bytes_read = port.read(response)?;

            Ok::<_, TransportError>(bytes_read)
        })
        .await?
    }
}
