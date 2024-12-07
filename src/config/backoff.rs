use std::time::Duration;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    /// Initial wait time
    #[serde(with = "humantime_serde")]
    pub initial_interval: Duration,
    /// Maximum wait time
    #[serde(with = "humantime_serde")]
    pub max_interval: Duration,
    /// Multiplier for each subsequent attempt
    pub multiplier: f64,
    /// Maximum number of attempts
    pub max_retries: u64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            initial_interval: Duration::from_millis(100),
            max_interval: Duration::from_secs(30),
            multiplier: 2.0,
            max_retries: 5,
        }
    }
}
