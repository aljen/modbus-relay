use std::time::Duration;

use serde::{Deserialize, Serialize};
use std::fmt;

use crate::{errors::ConfigValidationError, RelayError};

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
pub struct RelayConfig {
    // TCP settings
    pub tcp_bind_addr: String,
    pub tcp_bind_port: u16,

    // Serial port settings
    pub rtu_device: String,
    pub rtu_baud_rate: u32,
    pub data_bits: DataBits,
    pub parity: Parity,
    pub stop_bits: StopBits,

    /// Flow control settings for the serial port
    #[cfg(feature = "rts")]
    pub rtu_rts_type: RtsType,
    #[cfg(feature = "rts")]
    pub rtu_rts_delay_us: u64,

    pub rtu_flush_after_write: bool,

    /// Timeout for the entire transaction (request + response)
    #[serde(with = "duration_millis")]
    pub transaction_timeout: Duration,

    /// Timeout for individual read/write operations on serial port
    #[serde(with = "duration_millis")]
    pub serial_timeout: Duration,

    /// Maximum size of the request/response buffer
    pub max_frame_size: usize,

    /// Enable hexdump of frames in trace logs
    pub trace_frames: bool,
}

impl Default for RelayConfig {
    fn default() -> Self {
        Self {
            tcp_bind_addr: "0.0.0.0".to_string(),
            tcp_bind_port: 5000,

            rtu_device: "/dev/ttyAMA0".to_string(),
            rtu_baud_rate: 9600,
            data_bits: DataBits::default(), // 8
            parity: Parity::None,
            stop_bits: StopBits::One,

            #[cfg(feature = "rts")]
            rtu_rts_type: RtsType::Down,
            #[cfg(feature = "rts")]
            rtu_rts_delay_us: 3500,

            rtu_flush_after_write: true,

            transaction_timeout: Duration::from_secs(1),
            serial_timeout: Duration::from_millis(500),

            max_frame_size: 256,
            trace_frames: false,
        }
    }
}

impl RelayConfig {
    pub fn validate(&self) -> Result<(), RelayError> {
        // TCP validation
        if self.tcp_bind_addr.parse::<std::net::IpAddr>().is_err() {
            return Err(RelayError::Config(ConfigValidationError::tcp(format!(
                "Invalid TCP bind address: {}",
                self.tcp_bind_addr
            ))));
        }

        if !(1..=65535).contains(&self.tcp_bind_port) {
            return Err(RelayError::Config(ConfigValidationError::tcp(format!(
                "Invalid TCP port: {}",
                self.tcp_bind_port
            ))));
        }

        // Serial port validation
        if self.rtu_baud_rate == 0 || self.rtu_baud_rate > 921600 {
            return Err(RelayError::Config(ConfigValidationError::rtu(format!(
                "Invalid baud rate: {}",
                self.rtu_baud_rate
            ))));
        }

        // Timeout validation
        if self.transaction_timeout.as_millis() == 0 {
            return Err(RelayError::Config(ConfigValidationError::timing(
                "Transaction timeout cannot be 0".to_string(),
            )));
        }

        if self.serial_timeout.as_millis() == 0 {
            return Err(RelayError::Config(ConfigValidationError::timing(
                "Serial timeout cannot be 0".to_string(),
            )));
        }

        // RTS validation
        #[cfg(feature = "rts")]
        if self.rtu_rts_type != RtsType::None {
            if self.rtu_rts_delay_us > 10000 {
                return Err(RelayError::Config(ConfigValidationError::rtu(format!(
                    "RTS delay too large: {}µs",
                    self.rtu_rts_delay_us
                ))));
            }
        }

        // Buffer validation
        if self.max_frame_size < 8 {
            return Err(RelayError::Config(ConfigValidationError::rtu(format!(
                "Frame size too small: {} bytes (min 8)",
                self.max_frame_size
            ))));
        }

        if self.max_frame_size > 256 {
            return Err(RelayError::Config(ConfigValidationError::rtu(format!(
                "Frame size too large: {} bytes (max 256)",
                self.max_frame_size
            ))));
        }

        // Validate that transaction timeout is greater than serial timeout
        if self.transaction_timeout <= self.serial_timeout {
            return Err(RelayError::Config(ConfigValidationError::timing(format!(
                "Transaction timeout ({:?}) must be greater than serial timeout ({:?})",
                self.transaction_timeout, self.serial_timeout
            ))));
        }

        Ok(())
    }

    pub fn from_env() -> Result<Self, RelayError> {
        let config = Self {
            tcp_bind_addr: std::env::var("MODBUS_TCP_BIND_ADDR")
                .unwrap_or_else(|_| "0.0.0.0".to_string()),
            tcp_bind_port: std::env::var("MODBUS_TCP_BIND_PORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(5000),
            rtu_device: std::env::var("MODBUS_RTU_DEVICE")
                .unwrap_or_else(|_| "/dev/ttyAMA0".to_string()),
            rtu_baud_rate: std::env::var("MODBUS_RTU_BAUD_RATE")
                .ok()
                .and_then(|b| b.parse().ok())
                .unwrap_or(9600),
            data_bits: DataBits::default(),
            parity: Parity::None,
            stop_bits: StopBits::One,
            #[cfg(feature = "rts")]
            rtu_rts_type: RtsType::Up,
            #[cfg(feature = "rts")]
            rtu_rts_delay_us: std::env::var("MODBUS_RTS_DELAY_US")
                .ok()
                .and_then(|d| d.parse().ok())
                .unwrap_or(3500),
            rtu_flush_after_write: true,
            transaction_timeout: Duration::from_secs(1),
            serial_timeout: Duration::from_millis(500),
            max_frame_size: 256,
            trace_frames: false,
        };

        config.validate()?;
        Ok(config)
    }

    #[cfg(feature = "rts")]
    pub fn serial_port_info(&self) -> String {
        format!(
            "{}@{} {}{}{}{}",
            self.rtu_device,
            self.rtu_baud_rate,
            self.data_bits,
            self.parity,
            self.stop_bits,
            if self.rtu_rts_type != RtsType::None {
                format!(" (RTS delay: {}us)", self.rtu_rts_delay_us)
            } else {
                String::new()
            }
        )
    }

    #[cfg(not(feature = "rts"))]
    pub fn serial_port_info(&self) -> String {
        format!(
            "{}@{} {}{}{}",
            self.rtu_device, self.rtu_baud_rate, self.data_bits, self.parity, self.stop_bits,
        )
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
    fn test_valid_config() {
        let config = RelayConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_invalid_tcp_address() {
        let mut config = RelayConfig::default();
        config.tcp_bind_addr = "invalid".to_string();
        assert!(matches!(
            config.validate(),
            Err(RelayError::Config(ConfigValidationError::InvalidTcp(_)))
        ));
    }

    #[test]
    fn test_invalid_tcp_port() {
        let mut config = RelayConfig::default();
        config.tcp_bind_port = 0;
        assert!(matches!(
            config.validate(),
            Err(RelayError::Config(ConfigValidationError::InvalidTcp(_)))
        ));
    }

    #[test]
    fn test_invalid_baud_rate() {
        let mut config = RelayConfig::default();
        config.rtu_baud_rate = 0;
        assert!(matches!(
            config.validate(),
            Err(RelayError::Config(ConfigValidationError::InvalidRtu(_)))
        ));
    }

    #[test]
    fn test_invalid_timeouts() {
        let mut config = RelayConfig::default();
        config.transaction_timeout = Duration::from_secs(0);
        assert!(matches!(
            config.validate(),
            Err(RelayError::Config(ConfigValidationError::InvalidTiming(_)))
        ));

        config.transaction_timeout = Duration::from_secs(1);
        config.serial_timeout = Duration::from_secs(2);
        assert!(matches!(
            config.validate(),
            Err(RelayError::Config(ConfigValidationError::InvalidTiming(_)))
        ));
    }

    #[test]
    fn test_invalid_frame_size() {
        let mut config = RelayConfig::default();
        config.max_frame_size = 4;
        assert!(matches!(
            config.validate(),
            Err(RelayError::Config(ConfigValidationError::InvalidRtu(_)))
        ));

        config.max_frame_size = 300;
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
        assert_eq!(config.tcp_bind_port, 8080);
        assert_eq!(config.rtu_baud_rate, 19200);
    }
}
