use std::time::Duration;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub bind_addr: String,
    pub bind_port: u16,
    #[serde(with = "humantime_serde")]
    pub keep_alive: Duration,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            bind_addr: "0.0.0.0".to_string(),
            bind_port: 5000,
            keep_alive: Duration::from_secs(60),
        }
    }
}
