use std::time::Duration;

use serde::{Deserialize, Serialize};

use super::BackoffConfig;

/// Configuration for managing connections
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    /// Maximum number of concurrent connections
    pub max_connections: u64,
    /// Time after which an idle connection will be closed
    #[serde(with = "humantime_serde")]
    pub idle_timeout: Duration,
    /// Time after which a connection with errors will be closed
    #[serde(with = "humantime_serde")]
    pub error_timeout: Duration,
    /// Timeout for establishing a connection
    #[serde(with = "humantime_serde")]
    pub connect_timeout: Duration,
    /// Limits for specific IP addresses
    pub per_ip_limits: Option<u64>,
    /// Parameters for backoff strategy
    pub backoff: BackoffConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            max_connections: 100,
            idle_timeout: Duration::from_secs(60),
            error_timeout: Duration::from_secs(300),
            connect_timeout: Duration::from_secs(5),
            per_ip_limits: Some(10),
            backoff: BackoffConfig {
                initial_interval: Duration::from_millis(100),
                max_interval: Duration::from_secs(30),
                multiplier: 2.0,
                max_retries: 5,
            },
        }
    }
}
