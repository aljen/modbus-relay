use std::{path::Path, time::Duration};

use serde::{Deserialize, Serialize};
use std::fmt;
use tracing::{info, level_filters::LevelFilter};

use crate::{
    errors::{ConfigValidationError, InitializationError},
    RelayError,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogConfig {
    /// Enable trace-level logging for frame contents
    #[serde(default)]
    pub trace_frames: bool,

    /// Minimum log level for console output
    #[serde(default = "default_log_level")]
    pub log_level: String,

    /// Whether to include source code location in logs
    #[serde(default = "default_true")]
    pub include_location: bool,
}

impl Default for LogConfig {
    fn default() -> Self {
        Self {
            trace_frames: false,
            log_level: default_log_level(),
            include_location: true,
        }
    }
}

fn default_log_level() -> String {
    "info".to_string()
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Parity {
    None,
    Odd,
    Even,
}

impl From<Parity> for serialport::Parity {
    fn from(parity: Parity) -> Self {
        match parity {
            Parity::None => serialport::Parity::None,
            Parity::Odd => serialport::Parity::Odd,
            Parity::Even => serialport::Parity::Even,
        }
    }
}

impl fmt::Display for Parity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Parity::None => write!(f, "none"),
            Parity::Odd => write!(f, "odd"),
            Parity::Even => write!(f, "even"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StopBits {
    One,
    Two,
}

impl From<StopBits> for serialport::StopBits {
    fn from(stop_bits: StopBits) -> Self {
        match stop_bits {
            StopBits::One => serialport::StopBits::One,
            StopBits::Two => serialport::StopBits::Two,
        }
    }
}

impl fmt::Display for StopBits {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StopBits::One => write!(f, "1"),
            StopBits::Two => write!(f, "2"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct DataBits(u8);

impl DataBits {
    pub fn new(bits: u8) -> Option<Self> {
        match bits {
            5..=8 => Some(Self(bits)),
            _ => None,
        }
    }

    pub fn get(&self) -> u8 {
        self.0
    }
}

impl Default for DataBits {
    fn default() -> Self {
        Self(8)
    }
}

impl From<DataBits> for serialport::DataBits {
    fn from(data_bits: DataBits) -> Self {
        match data_bits.0 {
            5 => serialport::DataBits::Five,
            6 => serialport::DataBits::Six,
            7 => serialport::DataBits::Seven,
            8 => serialport::DataBits::Eight,
            _ => unreachable!("DataBits constructor ensures valid values"),
        }
    }
}

impl fmt::Display for DataBits {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RtsType {
    /// RTS disabled
    None,
    /// RTS = High during transmission
    Up,
    /// RTS = LOW during transmission
    Down,
}

impl RtsType {
    pub fn to_signal_level(&self, is_transmitting: bool) -> bool {
        match self {
            RtsType::None => false,
            RtsType::Up => is_transmitting,
            RtsType::Down => !is_transmitting,
        }
    }
}

impl std::fmt::Display for RtsType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RtsType::None => write!(f, "none"),
            RtsType::Up => write!(f, "up"),
            RtsType::Down => write!(f, "down"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TcpConfig {
    pub bind_addr: String,
    pub bind_port: u16,
}

impl Default for TcpConfig {
    fn default() -> Self {
        Self {
            bind_addr: "0.0.0.0".to_string(),
            bind_port: 5000,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RtuConfig {
    pub device: String,
    pub baud_rate: u32,
    pub data_bits: DataBits,
    pub parity: Parity,
    pub stop_bits: StopBits,

    /// Flow control settings for the serial port
    #[cfg(feature = "rts")]
    pub rts_type: RtsType,
    #[cfg(feature = "rts")]
    pub rts_delay_us: u64,

    pub flush_after_write: bool,
}

impl Default for RtuConfig {
    fn default() -> Self {
        Self {
            device: "/dev/ttyAMA0".to_string(),
            baud_rate: 9600,
            data_bits: DataBits::default(),
            parity: Parity::None,
            stop_bits: StopBits::One,

            #[cfg(feature = "rts")]
            rts_type: RtsType::Down,
            #[cfg(feature = "rts")]
            rts_delay_us: 3500,

            flush_after_write: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionConfig {
    /// Timeout for the entire transaction (request + response)
    #[serde(with = "duration_millis")]
    pub transaction_timeout: Duration,

    /// Timeout for individual read/write operations on serial port
    #[serde(with = "duration_millis")]
    pub serial_timeout: Duration,

    /// Maximum size of the request/response buffer
    pub max_frame_size: usize,
}

impl Default for ConnectionConfig {
    fn default() -> Self {
        Self {
            transaction_timeout: Duration::from_secs(1),
            serial_timeout: Duration::from_millis(500),
            max_frame_size: 256,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpConfig {
    /// Enable HTTP API
    pub enabled: bool,
    /// HTTP server port
    pub port: u16,
    /// Enable metrics collection
    pub metrics_enabled: bool,
}

impl Default for HttpConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            port: 8081,
            metrics_enabled: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    /// Enable trace-level logging for frame contents
    #[serde(default)]
    pub trace_frames: bool,

    /// Minimum log level for console output
    #[serde(default = "default_log_level")]
    pub log_level: String,

    /// Whether to include source code location in logs
    #[serde(default = "default_true")]
    pub include_location: bool,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            trace_frames: false,
            log_level: default_log_level(),
            include_location: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelayConfig {
    /// TCP server configuration
    #[serde(default)]
    pub tcp: TcpConfig,

    /// RTU client configuration
    #[serde(default)]
    pub rtu: RtuConfig,

    /// Connection management configuration
    #[serde(default)]
    pub connection: ConnectionConfig,

    /// HTTP API configuration
    #[serde(default)]
    pub http: HttpConfig,

    /// Logging configuration
    #[serde(default)]
    pub logging: LoggingConfig,
}

impl Default for RelayConfig {
    fn default() -> Self {
        Self {
            tcp: TcpConfig::default(),
            rtu: RtuConfig::default(),
            connection: ConnectionConfig::default(),
            http: HttpConfig::default(),
            logging: LoggingConfig::default(),
        }
    }
}

impl RelayConfig {
    /// Loads configuration with the following precedence:
    /// 1. Environment variables
    /// 2. Config file
    /// 3. Default values
    pub fn load(config_path: Option<&Path>) -> Result<Self, RelayError> {
        // Start with default config
        let mut config = if let Some(path) = config_path {
            if path.exists() {
                info!("Loading config from {}", path.display());
                let content = std::fs::read_to_string(path).map_err(|e| {
                    RelayError::Init(InitializationError::config(format!(
                        "Failed to read config file: {}",
                        e
                    )))
                })?;
                serde_json::from_str(&content).map_err(|e| {
                    RelayError::Init(InitializationError::config(format!(
                        "Failed to parse config file: {}",
                        e
                    )))
                })?
            } else {
                info!("Config file not found, using defaults");
                Self::default()
            }
        } else {
            info!("No config file specified, using defaults");
            Self::default()
        };

        // Override with environment variables if present
        if let Ok(addr) = std::env::var("MODBUS_TCP_BIND_ADDR") {
            config.tcp.bind_addr = addr;
        }

        if let Ok(port_str) = std::env::var("MODBUS_TCP_BIND_PORT") {
            if let Ok(port) = port_str.parse() {
                config.tcp.bind_port = port;
            }
        }

        if let Ok(device) = std::env::var("MODBUS_RTU_DEVICE") {
            config.rtu.device = device;
        }

        if let Ok(baud_str) = std::env::var("MODBUS_RTU_BAUD_RATE") {
            if let Ok(baud) = baud_str.parse() {
                config.rtu.baud_rate = baud;
            }
        }

        // RTS settings
        #[cfg(feature = "rts")]
        if let Ok(delay_str) = std::env::var("MODBUS_RTS_DELAY_US") {
            if let Ok(delay) = delay_str.parse() {
                config.rtu.rts_delay_us = delay;
            }
        }

        // Logging settings
        if let Ok(level) = std::env::var("MODBUS_LOG_LEVEL") {
            config.logging.log_level = level;
        }

        // Validate the final configuration
        config.validate()?;

        Ok(config)
    }

    pub fn validate(&self) -> Result<(), RelayError> {
        // Validate logging configuration first
        self.logging.validate().map_err(RelayError::Init)?;

        // TCP validation
        if self.tcp.bind_addr.parse::<std::net::IpAddr>().is_err() {
            return Err(RelayError::Config(ConfigValidationError::tcp(format!(
                "Invalid TCP bind address: {}",
                self.tcp.bind_addr
            ))));
        }

        if !(1..=65535).contains(&self.tcp.bind_port) {
            return Err(RelayError::Config(ConfigValidationError::tcp(format!(
                "Invalid TCP port: {}",
                self.tcp.bind_port
            ))));
        }

        // Serial port validation
        if self.rtu.baud_rate == 0 || self.rtu.baud_rate > 921600 {
            return Err(RelayError::Config(ConfigValidationError::rtu(format!(
                "Invalid baud rate: {}",
                self.rtu.baud_rate
            ))));
        }

        // Timeout validation
        if self.connection.transaction_timeout.as_millis() == 0 {
            return Err(RelayError::Config(ConfigValidationError::timing(
                "Transaction timeout cannot be 0".to_string(),
            )));
        }

        if self.connection.serial_timeout.as_millis() == 0 {
            return Err(RelayError::Config(ConfigValidationError::timing(
                "Serial timeout cannot be 0".to_string(),
            )));
        }

        // RTS validation
        #[cfg(feature = "rts")]
        if self.rtu.rts_type != RtsType::None {
            if self.rtu.rts_delay_us > 10000 {
                return Err(RelayError::Config(ConfigValidationError::rtu(format!(
                    "RTS delay too large: {}µs",
                    self.rtu.rts_delay_us
                ))));
            }
        }

        // Buffer validation
        if self.connection.max_frame_size < 8 {
            return Err(RelayError::Config(ConfigValidationError::rtu(format!(
                "Frame size too small: {} bytes (min 8)",
                self.connection.max_frame_size
            ))));
        }

        if self.connection.max_frame_size > 256 {
            return Err(RelayError::Config(ConfigValidationError::rtu(format!(
                "Frame size too large: {} bytes (max 256)",
                self.connection.max_frame_size
            ))));
        }

        // Validate that transaction timeout is greater than serial timeout
        if self.connection.transaction_timeout <= self.connection.serial_timeout {
            return Err(RelayError::Config(ConfigValidationError::timing(format!(
                "Transaction timeout ({:?}) must be greater than serial timeout ({:?})",
                self.connection.transaction_timeout, self.connection.serial_timeout
            ))));
        }

        Ok(())
    }

    pub fn from_env() -> Result<Self, RelayError> {
        let config = Self {
            tcp: TcpConfig {
                bind_addr: std::env::var("MODBUS_TCP_BIND_ADDR")
                    .unwrap_or_else(|_| "0.0.0.0".to_string()),
                bind_port: std::env::var("MODBUS_TCP_BIND_PORT")
                    .ok()
                    .and_then(|p| p.parse().ok())
                    .unwrap_or(5000),
            },
            rtu: RtuConfig {
                device: std::env::var("MODBUS_RTU_DEVICE")
                    .unwrap_or_else(|_| "/dev/ttyAMA0".to_string()),
                baud_rate: std::env::var("MODBUS_RTU_BAUD_RATE")
                    .ok()
                    .and_then(|b| b.parse().ok())
                    .unwrap_or(9600),
                ..Default::default()
            },
            connection: ConnectionConfig::default(),
            http: HttpConfig::default(),
            logging: LoggingConfig::default(),
        };

        // Validate entire config including logging
        config.validate()?;

        // Double check logging config specifically since it's critical
        config.logging.validate().map_err(RelayError::Init)?;

        Ok(config)
    }

    #[cfg(feature = "rts")]
    pub fn serial_port_info(&self) -> String {
        format!(
            "{}@{} {}{}{}{}",
            self.rtu.device,
            self.rtu.baud_rate,
            self.rtu.data_bits,
            self.rtu.parity,
            self.rtu.stop_bits,
            if self.rtu.rts_type != RtsType::None {
                format!(" (RTS delay: {}us)", self.rtu.rts_delay_us)
            } else {
                String::new()
            }
        )
    }

    #[cfg(not(feature = "rts"))]
    pub fn serial_port_info(&self) -> String {
        format!(
            "{}@{} {}{}{}",
            self.rtu.device, self.rtu.baud_rate, self.rtu.data_bits, self.rtu.parity, self.rtu.stop_bits,
        )
    }
}

impl LoggingConfig {
    /// Validates the configuration
    pub fn validate(&self) -> Result<(), InitializationError> {
        // Validate log level
        match self.log_level.to_lowercase().as_str() {
            "error" | "warn" | "info" | "debug" | "trace" => Ok(()),
            _ => Err(InitializationError::logging(format!(
                "Invalid log level: {}",
                self.log_level
            ))),
        }
    }

    pub fn get_level_filter(&self) -> LevelFilter {
        match self.log_level.to_lowercase().as_str() {
            "error" => LevelFilter::ERROR,
            "warn" => LevelFilter::WARN,
            "info" => LevelFilter::INFO,
            "debug" => LevelFilter::DEBUG,
            "trace" => LevelFilter::TRACE,
            _ => LevelFilter::INFO,
        }
    }
}

// Helper module for Duration serialization in milliseconds
mod duration_millis {
    use serde::{Deserialize, Deserializer, Serializer};
    use std::time::Duration;

    pub fn serialize<S>(duration: &Duration, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u64(duration.as_millis() as u64)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Duration, D::Error>
    where
        D: Deserializer<'de>,
    {
        let millis = u64::deserialize(deserializer)?;
        Ok(Duration::from_millis(millis))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_config_load_defaults() {
        let config = RelayConfig::load(None).unwrap();
        assert_eq!(config.tcp.bind_addr, "0.0.0.0");
        assert_eq!(config.tcp.bind_port, 5000);
    }

    #[test]
    fn test_config_load_from_env() {
        std::env::set_var("MODBUS_TCP_BIND_ADDR", "127.0.0.1");
        std::env::set_var("MODBUS_TCP_BIND_PORT", "8080");
        std::env::set_var("MODBUS_RTU_DEVICE", "/dev/ttyUSB0");

        let config = RelayConfig::load(None).unwrap();

        assert_eq!(config.tcp.bind_addr, "127.0.0.1");
        assert_eq!(config.tcp.bind_port, 8080);
        assert_eq!(config.rtu.device, "/dev/ttyUSB0");

        // Cleanup
        std::env::remove_var("MODBUS_TCP_BIND_ADDR");
        std::env::remove_var("MODBUS_TCP_BIND_PORT");
        std::env::remove_var("MODBUS_RTU_DEVICE");
    }

    #[test]
    fn test_config_load_from_file() {
        let temp_dir = tempdir::TempDir::new("modbusrelay-test").unwrap();
        let config_path = temp_dir.path().join("config.json");

        let test_config = RelayConfig {
            tcp: TcpConfig {
                bind_addr: "192.168.1.1".to_string(),
                bind_port: 9999,
            },
            ..Default::default()
        };

        std::fs::write(
            &config_path,
            serde_json::to_string_pretty(&test_config).unwrap(),
        )
        .unwrap();

        let loaded_config = RelayConfig::load(Some(&config_path)).unwrap();
        assert_eq!(loaded_config.tcp.bind_addr, "192.168.1.1");
        assert_eq!(loaded_config.tcp.bind_port, 9999);
    }

    #[test]
    fn test_config_env_overrides_file() {
        let temp_dir = tempdir::TempDir::new("modbusrelay-test").unwrap();
        let config_path = temp_dir.path().join("config.json");

        let test_config = RelayConfig {
            tcp: TcpConfig {
                bind_addr: "192.168.1.1".to_string(),
                bind_port: 9999,
            },
            ..Default::default()
        };

        std::fs::write(
            &config_path,
            serde_json::to_string_pretty(&test_config).unwrap(),
        )
        .unwrap();

        std::env::set_var("MODBUS_TCP_BIND_ADDR", "127.0.0.1");

        let loaded_config = RelayConfig::load(Some(&config_path)).unwrap();
        assert_eq!(loaded_config.tcp.bind_addr, "127.0.0.1"); // From env
        assert_eq!(loaded_config.tcp.bind_port, 9999); // From file

        std::env::remove_var("MODBUS_TCP_BIND_ADDR");
    }

    #[test]
    fn test_valid_config() {
        let config = RelayConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_invalid_tcp_address() {
        let mut config = RelayConfig::default();
        config.tcp.bind_addr = "invalid".to_string();
        assert!(matches!(
            config.validate(),
            Err(RelayError::Config(ConfigValidationError::InvalidTcp(_)))
        ));
    }

    #[test]
    fn test_invalid_tcp_port() {
        let mut config = RelayConfig::default();
        config.tcp.bind_port = 0;
        assert!(matches!(
            config.validate(),
            Err(RelayError::Config(ConfigValidationError::InvalidTcp(_)))
        ));
    }

    #[test]
    fn test_invalid_baud_rate() {
        let mut config = RelayConfig::default();
        config.rtu.baud_rate = 0;
        assert!(matches!(
            config.validate(),
            Err(RelayError::Config(ConfigValidationError::InvalidRtu(_)))
        ));
    }

    #[test]
    fn test_invalid_timeouts() {
        let mut config = RelayConfig::default();
        config.connection.transaction_timeout = Duration::from_secs(0);
        assert!(matches!(
            config.validate(),
            Err(RelayError::Config(ConfigValidationError::InvalidTiming(_)))
        ));

        config.connection.transaction_timeout = Duration::from_secs(1);
        config.connection.serial_timeout = Duration::from_secs(2);
        assert!(matches!(
            config.validate(),
            Err(RelayError::Config(ConfigValidationError::InvalidTiming(_)))
        ));
    }

    #[test]
    fn test_invalid_frame_size() {
        let mut config = RelayConfig::default();
        config.connection.max_frame_size = 4;
        assert!(matches!(
            config.validate(),
            Err(RelayError::Config(ConfigValidationError::InvalidRtu(_)))
        ));

        config.connection.max_frame_size = 300;
        assert!(matches!(
            config.validate(),
            Err(RelayError::Config(ConfigValidationError::InvalidRtu(_)))
        ));
    }

    #[test]
    fn test_config_from_env() {
        std::env::set_var("MODBUS_TCP_BIND_PORT", "8080");
        std::env::set_var("MODBUS_RTU_BAUD_RATE", "19200");

        let config = RelayConfig::from_env().unwrap();
        assert_eq!(config.tcp.bind_port, 8080);
        assert_eq!(config.rtu.baud_rate, 19200);
    }
}
