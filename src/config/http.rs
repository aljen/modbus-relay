use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    /// Enable HTTP API
    pub enabled: bool,
    /// HTTP server address
    pub bind_addr: String,
    /// HTTP server port
    pub bind_port: u16,
    /// Enable metrics collection
    pub metrics_enabled: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            enabled: true,
            bind_addr: "127.0.0.1".to_string(),
            bind_port: 8081,
            metrics_enabled: true,
        }
    }
}
