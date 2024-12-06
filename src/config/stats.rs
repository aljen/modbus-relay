use std::time::Duration;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(with = "humantime_serde")]
    pub cleanup_interval: Duration,
    #[serde(with = "humantime_serde")]
    pub idle_timeout: Duration,
    #[serde(with = "humantime_serde")]
    pub error_timeout: Duration,
    pub max_events_per_second: u32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            cleanup_interval: Duration::from_secs(60),
            idle_timeout: Duration::from_secs(300),
            error_timeout: Duration::from_secs(300),
            max_events_per_second: 10000,
        }
    }
}
