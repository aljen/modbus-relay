use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Enable HTTP API
    pub enabled: bool,
    /// HTTP server port
    pub port: u16,
    /// Enable metrics collection
    pub metrics_enabled: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            enabled: true,
            port: 8081,
            metrics_enabled: true,
        }
    }
}
