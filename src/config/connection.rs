use std::time::Duration;

use crate::ConfigValidationError;

use super::BackoffConfig;

/// Configuration for managing connections
#[derive(Debug, Clone)]
pub struct Config {
    /// Maximum number of concurrent connections
    pub max_connections: usize,
    /// Timeout for idle connections
    pub idle_timeout: Duration,
    /// Timeout for establishing a connection
    pub connect_timeout: Duration,
    /// Limits for specific IP addresses
    pub per_ip_limits: Option<usize>,
    /// Parameters for backoff strategy
    pub backoff: BackoffConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            max_connections: 100,
            idle_timeout: Duration::from_secs(60),
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

impl Config {
    pub fn validate(&self) -> Result<(), ConfigValidationError> {
        if self.max_connections == 0 {
            return Err(ConfigValidationError::connection(
                "max_connections cannot be 0".to_string(),
            ));
        }

        if let Some(limit) = self.per_ip_limits {
            if limit == 0 {
                return Err(ConfigValidationError::connection(
                    "per_ip_limits cannot be 0".to_string(),
                ));
            }
            if limit > self.max_connections {
                return Err(ConfigValidationError::connection(format!(
                    "per_ip_limits ({}) cannot be greater than max_connections ({})",
                    limit, self.max_connections
                )));
            }
        }

        if self.idle_timeout.as_secs() == 0 {
            return Err(ConfigValidationError::connection(
                "idle_timeout cannot be 0".to_string(),
            ));
        }

        if self.connect_timeout.as_secs() == 0 {
            return Err(ConfigValidationError::connection(
                "connect_timeout cannot be 0".to_string(),
            ));
        }

        Ok(())
    }
}
