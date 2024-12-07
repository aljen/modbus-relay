use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use config::{Config as ConfigBuilder, ConfigError, Environment, File, FileFormat};

use super::{ConnectionConfig, HttpConfig, LoggingConfig, RtuConfig, TcpConfig};

/// Main application configuration
#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    /// TCP server configuration
    pub tcp: TcpConfig,

    /// RTU client configuration
    pub rtu: RtuConfig,

    /// HTTP API configuration
    pub http: HttpConfig,

    /// Logging configuration
    pub logging: LoggingConfig,

    /// Connection management configuration
    pub connection: ConnectionConfig,
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
            .set_default("rtu.rts_type", defaults.rtu.rts_type.to_string())?
            .set_default("rtu.rts_delay_us", defaults.rtu.rts_delay_us)?
            .set_default(
                "rtu.transaction_timeout",
                format!("{}s", defaults.rtu.transaction_timeout.as_secs()),
            )?
            .set_default(
                "rtu.serial_timeout",
                format!("{}s", defaults.rtu.serial_timeout.as_secs()),
            )?
            .set_default("rtu.max_frame_size", defaults.rtu.max_frame_size)?
            // HTTP configuration
            .set_default("http.enabled", defaults.http.enabled)?
            .set_default("http.bind_addr", defaults.http.bind_addr)?
            .set_default("http.bind_port", defaults.http.bind_port)?
            .set_default("http.metrics_enabled", defaults.http.metrics_enabled)?
            // Logging configuration
            .set_default("logging.log_dir", defaults.logging.log_dir)?
            .set_default("logging.trace_frames", defaults.logging.trace_frames)?
            .set_default("logging.level", defaults.logging.level)?
            .set_default("logging.format", defaults.logging.format)?
            .set_default(
                "logging.include_location",
                defaults.logging.include_location,
            )?
            .set_default("logging.thread_ids", defaults.logging.thread_ids)?
            .set_default("logging.thread_names", defaults.logging.thread_names)?
            // Connection configuration
            .set_default(
                "connection.max_connections",
                defaults.connection.max_connections,
            )?
            .set_default(
                "connection.idle_timeout",
                format!("{}s", defaults.connection.idle_timeout.as_secs()),
            )?
            .set_default(
                "connection.connect_timeout",
                format!("{}s", defaults.connection.connect_timeout.as_secs()),
            )?
            .set_default(
                "connection.per_ip_limits",
                defaults.connection.per_ip_limits,
            )?
            // Connection backoff configuration
            .set_default(
                "connection.backoff.initial_interval",
                format!(
                    "{}s",
                    defaults.connection.backoff.initial_interval.as_secs()
                ),
            )?
            .set_default(
                "connection.backoff.max_interval",
                format!("{}s", defaults.connection.backoff.max_interval.as_secs()),
            )?
            .set_default(
                "connection.backoff.multiplier",
                defaults.connection.backoff.multiplier,
            )?
            .set_default(
                "connection.backoff.max_retries",
                defaults.connection.backoff.max_retries,
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
                    .prefix_separator("_")
                    .separator("__")
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

        // Validate log level
        match config.logging.level.to_lowercase().as_str() {
            "error" | "warn" | "info" | "debug" | "trace" => {}
            _ => return Err(validation_error("Invalid log level")),
        }

        // Validate log format
        match config.logging.format.to_lowercase().as_str() {
            "pretty" | "json" => {}
            _ => return Err(validation_error("Invalid log format")),
        }

        // Validate connection configuration
        if config.connection.max_connections == 0 {
            return Err(validation_error("Maximum connections must be non-zero"));
        }
        if config.connection.idle_timeout.is_zero() {
            return Err(validation_error("Idle timeout must be non-zero"));
        }
        if config.connection.connect_timeout.is_zero() {
            return Err(validation_error("Connect timeout must be non-zero"));
        }
        if let Some(limit) = config.connection.per_ip_limits {
            if limit == 0 {
                return Err(validation_error("Per IP connection limit must be non-zero"));
            }
            if limit > config.connection.max_connections {
                return Err(validation_error(
                    "Per IP connection limit cannot exceed maximum connections",
                ));
            }
        }
        // Validate backoff configuration
        if config.connection.backoff.initial_interval.is_zero() {
            return Err(validation_error(
                "Backoff initial interval must be non-zero",
            ));
        }
        if config.connection.backoff.max_interval.is_zero() {
            return Err(validation_error("Backoff max interval must be non-zero"));
        }
        if config.connection.backoff.multiplier <= 0.0 {
            return Err(validation_error("Backoff multiplier must be positive"));
        }
        if config.connection.backoff.max_retries == 0 {
            return Err(validation_error("Backoff max retries must be non-zero"));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::{DataBits, Parity, RtsType, StopBits};

    use super::*;
    use std::{fs, time::Duration};
    use tempfile::tempdir;

    #[test]
    #[serial_test::serial]
    fn test_default_config() {
        let config = Config::new().unwrap();
        assert_eq!(config.tcp.bind_port, 502);
        assert_eq!(config.tcp.bind_addr, "127.0.0.1");
    }

    #[test]
    #[serial_test::serial]
    fn test_env_override() {
        std::env::set_var("MODBUS_RELAY_TCP__BIND_PORT", "5000");
        let config = Config::new().unwrap();
        assert_eq!(config.tcp.bind_port, 5000);
        std::env::remove_var("MODBUS_RELAY_TCP__BIND_PORT");
    }

    #[test]
    #[serial_test::serial]
    fn test_file_config() {
        let dir = tempdir().unwrap();
        let config_path = dir.path().join("config.yaml");

        fs::write(
            &config_path,
            r#"
            tcp:
              bind_port: 9000
              bind_addr: "192.168.1.100"
              keep_alive: "60s"
            rtu:
              device: "/dev/ttyAMA0"
              baud_rate: 9600
              data_bits: 8
              parity: "none"
              stop_bits: "one"
              flush_after_write: true
              rts_type: "down"
              rts_delay_us: 3500
              transaction_timeout: "5s"
              serial_timeout: "1s"
              max_frame_size: 256
            http:
              enabled: false
              bind_addr: "192.168.1.100"
              bind_port: 9080
              metrics_enabled: false
            logging:
              log_dir: "logs"
              trace_frames: false
              level: "trace"
              format: "pretty"
              include_location: false
              thread_ids: false
              thread_names: true
            connection:
              max_connections: 100
              idle_timeout: "60s"
              error_timeout: "300s"
              connect_timeout: "5s"
              per_ip_limits: 10
              backoff:
                # Initial wait time
                initial_interval: "100ms"
                # Maximum wait time
                max_interval: "30s"
                # Multiplier for each subsequent attempt
                multiplier: 2.0
                # Maximum number of attempts
                max_retries: 5
            "#,
        )
        .unwrap();

        let config = Config::from_file(config_path).unwrap();
        assert_eq!(config.tcp.bind_port, 9000);
        assert_eq!(config.tcp.bind_addr, "192.168.1.100");
        assert_eq!(config.tcp.keep_alive, Duration::from_secs(60));
        assert_eq!(config.rtu.device, "/dev/ttyAMA0");
        assert_eq!(config.rtu.baud_rate, 9600);
        assert_eq!(config.rtu.data_bits, DataBits::new(8).unwrap());
        assert_eq!(config.rtu.parity, Parity::None);
        assert_eq!(config.rtu.stop_bits, StopBits::One);
        assert!(config.rtu.flush_after_write);
        assert_eq!(config.rtu.rts_type, RtsType::Down);
        assert_eq!(config.rtu.rts_delay_us, 3500);
        assert_eq!(config.rtu.transaction_timeout, Duration::from_secs(5));
        assert_eq!(config.rtu.serial_timeout, Duration::from_secs(1));
        assert_eq!(config.rtu.max_frame_size, 256);
        assert!(!config.http.enabled);
        assert_eq!(config.http.bind_addr, "192.168.1.100");
        assert_eq!(config.http.bind_port, 9080);
        assert!(!config.http.metrics_enabled);
        assert_eq!(config.logging.log_dir, "logs");
        assert!(!config.logging.trace_frames);
        assert_eq!(config.logging.level, "trace");
        assert_eq!(config.logging.format, "pretty");
        assert!(!config.logging.include_location);
        assert!(!config.logging.thread_ids);
        assert!(config.logging.thread_names);
        assert_eq!(config.connection.max_connections, 100);
        assert_eq!(config.connection.idle_timeout, Duration::from_secs(60));
        assert_eq!(config.connection.error_timeout, Duration::from_secs(300));
        assert_eq!(config.connection.connect_timeout, Duration::from_secs(5));
        assert_eq!(config.connection.per_ip_limits, Some(10));
        assert_eq!(
            config.connection.backoff.initial_interval,
            Duration::from_millis(100)
        );
        assert_eq!(
            config.connection.backoff.max_interval,
            Duration::from_secs(30)
        );
        assert_eq!(config.connection.backoff.multiplier, 2.0);
        assert_eq!(config.connection.backoff.max_retries, 5);
    }

    #[test]
    #[serial_test::serial]
    fn test_validation() {
        std::env::set_var("MODBUS_RELAY_TCP__BIND_PORT", "0");
        assert!(Config::new().is_err());
        std::env::remove_var("MODBUS_RELAY_TCP__BIND_PORT");
    }
}
