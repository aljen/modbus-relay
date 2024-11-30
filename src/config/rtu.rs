use serde::{Deserialize, Serialize};
use crate::config::types::{DataBits, Parity, StopBits};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub device: String,
    pub baud_rate: u32,
    pub data_bits: DataBits,
    pub parity: Parity,
    pub stop_bits: StopBits,

    /// Flow control settings for the serial port
    #[cfg(feature = "rts")]
    pub rts_type: RtsType,
    #[cfg(feature = "rts")]
    pub rts_delay_us: u64,

    /// Whether to flush the serial port after writing
    pub flush_after_write: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            device: "/dev/ttyAMA0".to_string(),
            baud_rate: 9600,
            data_bits: DataBits::default(),
            parity: Parity::default(),
            stop_bits: StopBits::default(),
            #[cfg(feature = "rts")]
            rts_type: RtsType::default(),
            #[cfg(feature = "rts")]
            rts_delay_us: 3500,
            flush_after_write: true,
        }
    }
}

impl Config {
    pub fn serial_port_info(&self) -> String {
        format!(
            "{} ({} baud, {} data bits, {} parity, {} stop bits)",
            self.device,
            self.baud_rate,
            self.data_bits,
            self.parity,
            self.stop_bits
        )
    }
}
