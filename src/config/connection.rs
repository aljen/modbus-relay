use std::time::Duration;
use serde::{Deserialize, Serialize};
use humantime_serde;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Timeout for the entire transaction (request + response)
    #[serde(with = "humantime_serde")]
    pub transaction_timeout: Duration,

    /// Timeout for individual read/write operations on serial port
    #[serde(with = "humantime_serde")]
    pub serial_timeout: Duration,

    /// Maximum size of the request/response buffer
    pub max_frame_size: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            transaction_timeout: Duration::from_secs(5),
            serial_timeout: Duration::from_secs(1),
            max_frame_size: 256,
        }
    }
}
