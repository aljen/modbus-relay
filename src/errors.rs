use std::time::Duration;
use thiserror::Error;
use tokio::time::error::Elapsed;

#[derive(Error, Debug)]
pub enum RelayError {
    #[error("Transport error: {0}")]
    Transport(#[from] TransportError),

    #[error("Network error: {0}")]
    Network(#[from] std::io::Error),

    #[error("Protocol error: {kind} - {details}")]
    Protocol {
        kind: ProtocolErrorKind,
        details: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    #[error("Configuration error: {kind} - {details}")]
    Config {
        kind: ConfigErrorKind,
        details: String,
    },

    #[error("Frame error: {kind} - {details}")]
    Frame {
        kind: FrameErrorKind,
        details: String,
        frame_data: Option<Vec<u8>>,
    },

    #[error("Buffer overflow: requested {requested} bytes, max {max} bytes")]
    BufferOverflow { requested: usize, max: usize },

    #[error("CRC error: calculated={calculated:04X}, received={received:04X}, frame={frame_hex}")]
    InvalidCrc {
        calculated: u16,
        received: u16,
        frame_hex: String,
    },

    #[error("Client error: {kind} from {client_addr} - {details}")]
    Client {
        kind: ClientErrorKind,
        client_addr: std::net::SocketAddr,
        details: String,
    },
}

#[derive(Error, Debug)]
pub enum TransportError {
    #[error("Serial port error: {kind} on {port} - {details}")]
    Serial {
        kind: SerialErrorKind,
        port: String,
        details: String,
        #[source]
        source: Option<serialport::Error>,
    },

    #[error("I/O error during {operation}: {details}")]
    Io {
        operation: IoOperation,
        details: String,
        #[source]
        source: std::io::Error,
    },

    #[error("Transaction timeout after {elapsed:?}, limit was {limit:?}")]
    Timeout {
        elapsed: Duration,
        limit: Duration,
        #[source]
        source: Elapsed,
    },

    #[error("No response received after {attempts} attempts over {elapsed:?}")]
    NoResponse { attempts: u8, elapsed: Duration },

    #[error("RTS control failed: {details}")]
    RtsError {
        details: String,
        #[source]
        source: Option<std::io::Error>,
    },
}

#[derive(Debug)]
pub struct ValidationError {
    pub kind: ConfigErrorKind,
    pub details: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtocolErrorKind {
    InvalidFunction,
    InvalidDataAddress,
    InvalidDataValue,
    ServerFailure,
    Acknowledge,
    ServerBusy,
    GatewayPathUnavailable,
    GatewayTargetFailedToRespond,
    InvalidProtocolId,
    InvalidTransactionId,
    InvalidUnitId,
    InvalidPdu,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigErrorKind {
    InvalidTcpAddress,
    InvalidTcpPort,
    InvalidBaudRate,
    InvalidDataBits,
    InvalidParity,
    InvalidStopBits,
    InvalidTimeout,
    InvalidRtsSettings,
    InvalidFrameSize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameErrorKind {
    TooShort,
    TooLong,
    InvalidFormat,
    InvalidHeader,
    InvalidCrc,
    UnexpectedResponse,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientErrorKind {
    ConnectionLost,
    Timeout,
    InvalidRequest,
    TooManyRequests,
    TooManyConnections,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SerialErrorKind {
    OpenFailed,
    ReadFailed,
    WriteFailed,
    ConfigurationFailed,
    Disconnected,
    BufferOverflow,
    ParityError,
    FramingError,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IoOperation {
    Read,
    Write,
    Flush,
    Configure,
    Control,
}

impl std::fmt::Display for ConfigErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidTcpAddress => write!(f, "Invalid TCP address"),
            Self::InvalidTcpPort => write!(f, "Invalid TCP port"),
            Self::InvalidBaudRate => write!(f, "Invalid baud rate"),
            Self::InvalidDataBits => write!(f, "Invalid data bits"),
            Self::InvalidParity => write!(f, "Invalid parity"),
            Self::InvalidStopBits => write!(f, "Invalid stop bits"),
            Self::InvalidTimeout => write!(f, "Invalid timeout"),
            Self::InvalidRtsSettings => write!(f, "Invalid RTS settings"),
            Self::InvalidFrameSize => write!(f, "Invalid frame size"),
        }
    }
}

impl std::fmt::Display for FrameErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TooShort => write!(f, "Frame too short"),
            Self::TooLong => write!(f, "Frame too long"),
            Self::InvalidFormat => write!(f, "Invalid frame format"),
            Self::InvalidHeader => write!(f, "Invalid frame header"),
            Self::InvalidCrc => write!(f, "Invalid frame CRC"),
            Self::UnexpectedResponse => write!(f, "Unexpected response"),
        }
    }
}

impl std::fmt::Display for ClientErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ConnectionLost => write!(f, "Connection lost"),
            Self::Timeout => write!(f, "Timeout"),
            Self::InvalidRequest => write!(f, "Invalid request"),
            Self::TooManyRequests => write!(f, "Too many requests"),
            Self::TooManyConnections => write!(f, "Too many connections"),
        }
    }
}

impl std::fmt::Display for SerialErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::OpenFailed => write!(f, "Failed to open port"),
            Self::ReadFailed => write!(f, "Failed to read from port"),
            Self::WriteFailed => write!(f, "Failed to write to port"),
            Self::ConfigurationFailed => write!(f, "Failed to configure port"),
            Self::Disconnected => write!(f, "Port disconnected"),
            Self::BufferOverflow => write!(f, "Buffer overflow"),
            Self::ParityError => write!(f, "Parity error"),
            Self::FramingError => write!(f, "Framing error"),
        }
    }
}

impl std::fmt::Display for IoOperation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Read => write!(f, "read"),
            Self::Write => write!(f, "write"),
            Self::Flush => write!(f, "flush"),
            Self::Configure => write!(f, "configure"),
            Self::Control => write!(f, "control"),
        }
    }
}

impl std::fmt::Display for ProtocolErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidFunction => write!(f, "Invalid function code"),
            Self::InvalidDataAddress => write!(f, "Invalid data address"),
            Self::InvalidDataValue => write!(f, "Invalid data value"),
            Self::ServerFailure => write!(f, "Server device failure"),
            Self::Acknowledge => write!(f, "Acknowledge"),
            Self::ServerBusy => write!(f, "Server device busy"),
            Self::GatewayPathUnavailable => write!(f, "Gateway path unavailable"),
            Self::GatewayTargetFailedToRespond => {
                write!(f, "Gateway target device failed to respond")
            }
            Self::InvalidProtocolId => write!(f, "Invalid protocol ID"),
            Self::InvalidTransactionId => write!(f, "Invalid transaction ID"),
            Self::InvalidUnitId => write!(f, "Invalid unit ID"),
            Self::InvalidPdu => write!(f, "Invalid PDU format"),
        }
    }
}

impl ProtocolErrorKind {
    pub fn to_exception_code(&self) -> u8 {
        match self {
            Self::InvalidFunction => 0x01,
            Self::InvalidDataAddress => 0x02,
            Self::InvalidDataValue => 0x03,
            Self::ServerFailure => 0x04,
            Self::Acknowledge => 0x05,
            Self::ServerBusy => 0x06,
            Self::GatewayPathUnavailable => 0x0A,
            Self::GatewayTargetFailedToRespond => 0x0B,
            _ => 0x04, // Map unknown errors to server failure
        }
    }

    pub fn from_exception_code(code: u8) -> Option<Self> {
        match code {
            0x01 => Some(Self::InvalidFunction),
            0x02 => Some(Self::InvalidDataAddress),
            0x03 => Some(Self::InvalidDataValue),
            0x04 => Some(Self::ServerFailure),
            0x05 => Some(Self::Acknowledge),
            0x06 => Some(Self::ServerBusy),
            0x0A => Some(Self::GatewayPathUnavailable),
            0x0B => Some(Self::GatewayTargetFailedToRespond),
            _ => None,
        }
    }
}

impl RelayError {
    pub fn protocol(kind: ProtocolErrorKind, details: impl Into<String>) -> Self {
        RelayError::Protocol {
            kind,
            details: details.into(),
            source: None,
        }
    }

    pub fn protocol_with_source(
        kind: ProtocolErrorKind,
        details: impl Into<String>,
        source: impl Into<Box<dyn std::error::Error + Send + Sync>>,
    ) -> Self {
        RelayError::Protocol {
            kind,
            details: details.into(),
            source: Some(source.into()),
        }
    }

    pub fn config(kind: ConfigErrorKind, details: impl Into<String>) -> Self {
        RelayError::Config {
            kind,
            details: details.into(),
        }
    }

    pub fn frame(
        kind: FrameErrorKind,
        details: impl Into<String>,
        frame_data: Option<Vec<u8>>,
    ) -> Self {
        RelayError::Frame {
            kind,
            details: details.into(),
            frame_data,
        }
    }

    pub fn client(
        kind: ClientErrorKind,
        client_addr: std::net::SocketAddr,
        details: impl Into<String>,
    ) -> Self {
        RelayError::Client {
            kind,
            client_addr,
            details: details.into(),
        }
    }
}

impl From<serialport::Error> for TransportError {
    fn from(err: serialport::Error) -> Self {
        match err.kind {
            serialport::ErrorKind::NoDevice => TransportError::Serial {
                kind: SerialErrorKind::OpenFailed,
                port: err.to_string(),
                details: "Device not found".into(),
                source: Some(err),
            },
            serialport::ErrorKind::InvalidInput => TransportError::Serial {
                kind: SerialErrorKind::ConfigurationFailed,
                port: err.to_string(),
                details: "Invalid configuration".into(),
                source: Some(err),
            },
            serialport::ErrorKind::Io(io_err) => TransportError::Io {
                operation: match io_err {
                    std::io::ErrorKind::NotFound => IoOperation::Configure,
                    std::io::ErrorKind::PermissionDenied => IoOperation::Configure,
                    std::io::ErrorKind::TimedOut => IoOperation::Read,
                    std::io::ErrorKind::WriteZero => IoOperation::Write,
                    _ => IoOperation::Control,
                },
                details: io_err.to_string(),
                source: std::io::Error::new(io_err, err.description),
            },
            _ => TransportError::Serial {
                kind: SerialErrorKind::OpenFailed,
                port: err.to_string(),
                details: err.to_string(),
                source: Some(err),
            },
        }
    }
}

impl From<std::io::Error> for TransportError {
    fn from(err: std::io::Error) -> Self {
        TransportError::Io {
            operation: match err.kind() {
                std::io::ErrorKind::TimedOut => IoOperation::Read,
                std::io::ErrorKind::WouldBlock => IoOperation::Read,
                std::io::ErrorKind::WriteZero => IoOperation::Write,
                std::io::ErrorKind::Interrupted => IoOperation::Control,
                _ => IoOperation::Control,
            },
            details: err.to_string(),
            source: err,
        }
    }
}

impl From<Elapsed> for TransportError {
    fn from(err: Elapsed) -> Self {
        TransportError::Timeout {
            elapsed: Duration::from_secs(1), // W Elapsed nie ma duration(), używamy stałej
            limit: Duration::from_secs(1), // TODO(aljen): Pass the actual limit from configuration
            source: err,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};

    #[test]
    fn test_error_creation_and_display() {
        let client_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080);

        let err = RelayError::client(
            ClientErrorKind::Timeout,
            client_addr,
            "Connection timed out",
        );

        assert!(err.to_string().contains("127.0.0.1:8080"));
        assert!(err.to_string().contains("Connection timed out"));
    }

    #[test]
    fn test_protocol_error_conversion() {
        let err = ProtocolErrorKind::InvalidFunction;
        assert_eq!(err.to_exception_code(), 0x01);
        assert_eq!(
            ProtocolErrorKind::from_exception_code(0x01),
            Some(ProtocolErrorKind::InvalidFunction)
        );
    }

    #[test]
    fn test_transport_error_from_serial_error() {
        let serial_err =
            serialport::Error::new(serialport::ErrorKind::NoDevice, "Device not found");

        let transport_err = TransportError::from(serial_err);
        match transport_err {
            TransportError::Serial { kind, .. } => {
                assert_eq!(kind, SerialErrorKind::OpenFailed);
            }
            _ => panic!("Expected Serial error variant"),
        }
    }
}
