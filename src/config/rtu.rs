use std::time::Duration;

use serde::{Deserialize, Serialize};

use super::{DataBits, Parity, RtsType, StopBits};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub device: String,
    pub baud_rate: u32,
    pub data_bits: DataBits,
    pub parity: Parity,
    pub stop_bits: StopBits,

    /// Flow control settings for the serial port
    pub rts_type: RtsType,
    pub rts_delay_us: u64,

    /// Whether to flush the serial port after writing
    pub flush_after_write: bool,

    /// Timeout for the entire transaction (request + response)
    #[serde(with = "humantime_serde")]
    pub transaction_timeout: Duration,

    /// Timeout for individual read/write operations on serial port
    #[serde(with = "humantime_serde")]
    pub serial_timeout: Duration,

    /// Maximum size of the request/response buffer
    pub max_frame_size: u64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            device: "/dev/ttyAMA0".to_string(),
            baud_rate: 9600,
            data_bits: DataBits::default(),
            parity: Parity::default(),
            stop_bits: StopBits::default(),
            rts_type: RtsType::default(),
            rts_delay_us: 3500,
            flush_after_write: true,
            transaction_timeout: Duration::from_secs(5),
            serial_timeout: Duration::from_secs(1),
            max_frame_size: 256,
        }
    }
}

impl Config {
    pub fn serial_port_info(&self) -> String {
        format!(
            "{} ({} baud, {} data bits, {} parity, {} stop bits)",
            self.device, self.baud_rate, self.data_bits, self.parity, self.stop_bits
        )
    }
}
