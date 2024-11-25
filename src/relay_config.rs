use std::time::Duration;

use serde::{Deserialize, Serialize};
use std::fmt;

use crate::{errors::ValidationError, ConfigErrorKind};

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

#[derive(Debug, Serialize, Deserialize)]
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
            rtu_rts_type: RtsType::Up,
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
    pub fn validate(&self) -> Result<(), ValidationError> {
        // TCP validation
        if !self.tcp_bind_addr.parse::<std::net::IpAddr>().is_ok() {
            return Err(ValidationError {
                kind: ConfigErrorKind::InvalidTcpAddress,
                details: format!("Invalid TCP bind address: {}", self.tcp_bind_addr),
            });
        }

        if !(1..=65535).contains(&self.tcp_bind_port) {
            return Err(ValidationError {
                kind: ConfigErrorKind::InvalidTcpPort,
                details: format!("Invalid TCP port: {}", self.tcp_bind_port),
            });
        }

        // Serial port validation
        if self.rtu_baud_rate == 0 || self.rtu_baud_rate > 921600 {
            return Err(ValidationError {
                kind: ConfigErrorKind::InvalidBaudRate,
                details: format!("Invalid baud rate: {}", self.rtu_baud_rate),
            });
        }

        // Timeout validation
        if self.transaction_timeout.as_millis() == 0 {
            return Err(ValidationError {
                kind: ConfigErrorKind::InvalidTimeout,
                details: "Transaction timeout cannot be 0".to_string(),
            });
        }

        if self.serial_timeout.as_millis() == 0 {
            return Err(ValidationError {
                kind: ConfigErrorKind::InvalidTimeout,
                details: "Serial timeout cannot be 0".to_string(),
            });
        }

        // RTS validation
        #[cfg(feature = "rts")]
        if self.rtu_rts_type != RtsType::None {
            if self.rtu_rts_delay_us > 10000 {
                return Err(ValidationError {
                    kind: ConfigErrorKind::InvalidRtsSettings,
                    details: format!("RTS delay too large: {}µs", self.rtu_rts_delay_us),
                });
            }
        }

        // Buffer validation
        if self.max_frame_size < 8 {
            return Err(ValidationError {
                kind: ConfigErrorKind::InvalidFrameSize,
                details: format!(
                    "Frame size too small: {} bytes (min 8)",
                    self.max_frame_size
                ),
            });
        }

        if self.max_frame_size > 256 {
            return Err(ValidationError {
                kind: ConfigErrorKind::InvalidFrameSize,
                details: format!(
                    "Frame size too large: {} bytes (max 256)",
                    self.max_frame_size
                ),
            });
        }

        Ok(())
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
