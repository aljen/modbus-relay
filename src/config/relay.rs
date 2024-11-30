use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use config::{Config as ConfigBuilder, ConfigError, Environment, File, FileFormat};

use super::{HttpConfig, LoggingConfig, RtuConfig, TcpConfig};

/// Main application configuration
#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// TCP server configuration
    #[serde(default)]
    pub tcp: TcpConfig,

    /// RTU client configuration
    #[serde(default)]
    pub rtu: RtuConfig,

    /// HTTP API configuration
    #[serde(default)]
    pub http: HttpConfig,

    /// Logging configuration
    #[serde(default)]
    pub logging: LoggingConfig,
}

impl Config {
    /// Default configuration directory
    pub const CONFIG_DIR: &'static str = "config";

    /// Environment variable prefix
    const ENV_PREFIX: &'static str = "MODBUS_RELAY";

    /// Build configuration using the following priority (highest to lowest):
    /// 1. Environment variables (MODBUS_RELAY_*)
    /// 2. Local configuration file (config/local.yaml)
    /// 3. Environment specific file (config/{env}.yaml)
    /// 4. Default configuration (config/default.yaml)
    /// 5. Built-in defaults
    pub fn new() -> Result<Self, ConfigError> {
        let environment = std::env::var("RUN_MODE").unwrap_or_else(|_| "development".into());

        // Start with built-in defaults
        let defaults = Config::default();

        let mut builder = ConfigBuilder::builder();

        // Set defaults for each field manually
        builder = builder
            // TCP configuration
            .set_default("tcp.bind_addr", defaults.tcp.bind_addr)?
            .set_default("tcp.bind_port", defaults.tcp.bind_port)?
            // RTU configuration
            .set_default("rtu.device", defaults.rtu.device)?
            .set_default("rtu.baud_rate", defaults.rtu.baud_rate)?
            .set_default("rtu.data_bits", defaults.rtu.data_bits.to_string())?
            .set_default("rtu.parity", defaults.rtu.parity.to_string())?
            .set_default("rtu.stop_bits", defaults.rtu.stop_bits.to_string())?
            .set_default("rtu.flush_after_write", defaults.rtu.flush_after_write)?
            .set_default(
                "rtu.transaction_timeout",
                format!("{}s", defaults.rtu.transaction_timeout.as_secs()),
            )?
            .set_default(
                "rtu.serial_timeout",
                format!("{}s", defaults.rtu.serial_timeout.as_secs()),
            )?
            .set_default("rtu.max_frame_size", defaults.rtu.max_frame_size as i64)?
            // HTTP configuration
            .set_default("http.enabled", defaults.http.enabled)?
            .set_default("http.port", defaults.http.port)?
            .set_default("http.metrics_enabled", defaults.http.metrics_enabled)?
            // Logging configuration
            .set_default("logging.trace_frames", defaults.logging.trace_frames)?
            .set_default("logging.log_level", defaults.logging.log_level)?
            .set_default("logging.format", defaults.logging.format)?
            .set_default(
                "logging.include_location",
                defaults.logging.include_location,
            )?;

        let config = builder
            // Load default config file
            .add_source(File::new(
                &format!("{}/default", Self::CONFIG_DIR),
                FileFormat::Yaml,
            ))
            // Load environment specific config
            .add_source(
                File::new(
                    &format!("{}/{}", Self::CONFIG_DIR, environment),
                    FileFormat::Yaml,
                )
                .required(false),
            )
            // Load local overrides
            .add_source(
                File::new(&format!("{}/local", Self::CONFIG_DIR), FileFormat::Yaml).required(false),
            )
            // Add environment variables
            .add_source(
                Environment::with_prefix(Self::ENV_PREFIX)
                    .separator("_")
                    .try_parsing(true),
            )
            .build()?;

        // Deserialize and validate
        let config = config.try_deserialize()?;
        Self::validate(&config)?;

        Ok(config)
    }

    /// Load configuration from a specific file
    pub fn from_file(path: PathBuf) -> Result<Self, ConfigError> {
        let config = ConfigBuilder::builder()
            // Load the specified config file
            .add_source(File::from(path))
            // Add env vars as overrides
            .add_source(
                Environment::with_prefix(Self::ENV_PREFIX)
                    .separator("_")
                    .try_parsing(true),
            )
            .build()?;

        let config = config.try_deserialize()?;
        Self::validate(&config)?;

        Ok(config)
    }

    /// Validate configuration
    pub fn validate(config: &Self) -> Result<(), ConfigError> {
        // Helper to convert validation errors
        fn validation_error(msg: &str) -> ConfigError {
            ConfigError::Message(msg.to_string())
        }

        // Validate TCP configuration
        if config.tcp.bind_addr.is_empty() {
            return Err(validation_error("TCP bind address must not be empty"));
        }
        if config.tcp.bind_port == 0 {
            return Err(validation_error("TCP port must be non-zero"));
        }

        // Validate RTU configuration
        if config.rtu.device.is_empty() {
            return Err(validation_error("RTU device must not be empty"));
        }
        if config.rtu.baud_rate == 0 {
            return Err(validation_error("RTU baud rate must be non-zero"));
        }

        // Validate connection configuration
        if config.rtu.transaction_timeout.is_zero() {
            return Err(validation_error("Transaction timeout must be non-zero"));
        }
        if config.rtu.serial_timeout.is_zero() {
            return Err(validation_error("Serial timeout must be non-zero"));
        }
        if config.rtu.max_frame_size == 0 {
            return Err(validation_error("Max frame size must be non-zero"));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_default_config() {
        let config = Config::new().unwrap();
        assert_eq!(config.tcp.bind_port, 502);
        assert_eq!(config.tcp.bind_addr, "127.0.0.1");
    }

    #[test]
    fn test_env_override() {
        std::env::set_var("MODBUS_RELAY_TCP_BIND_PORT", "8080");
        let config = Config::new().unwrap();
        assert_eq!(config.tcp.bind_port, 8080);
        std::env::remove_var("MODBUS_RELAY_TCP_BIND_PORT");
    }

    #[test]
    fn test_file_config() {
        let dir = tempdir().unwrap();
        let config_path = dir.path().join("config.yaml");

        fs::write(
            &config_path,
            r#"
            tcp:
              bind_port: 9000
              bind_addr: "192.168.1.100"
            "#,
        )
        .unwrap();

        let config = Config::from_file(config_path).unwrap();
        assert_eq!(config.tcp.bind_port, 9000);
        assert_eq!(config.tcp.bind_addr, "192.168.1.100");
    }

    #[test]
    fn test_validation() {
        std::env::set_var("MODBUS_RELAY_TCP_BIND_PORT", "0");
        assert!(Config::new().is_err());
        std::env::remove_var("MODBUS_RELAY_TCP_BIND_PORT");
    }
}
