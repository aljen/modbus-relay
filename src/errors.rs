use std::time::Duration;
use thiserror::Error;
use tokio::time::error::Elapsed;

#[derive(Error, Debug)]
pub enum RelayError {
    #[error("Transport error: {0}")]
    Transport(#[from] TransportError),

    #[error("Protocol error: {kind} - {details}")]
    Protocol {
        kind: ProtocolErrorKind,
        details: String,
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    #[error("Configuration error: {0}")]
    Config(#[from] ConfigValidationError),

    #[error("Frame error: {0}")]
    Frame(#[from] FrameError),

    #[error("Connection error: {0}")]
    Connection(#[from] ConnectionError),

    #[error("Client error: {kind} from {client_addr} - {details}")]
    Client {
        kind: ClientErrorKind,
        client_addr: std::net::SocketAddr,
        details: String,
    },

    #[error("Initialization error: {0}")]
    Init(#[from] InitializationError),
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

    #[error("Network error: {0}")]
    Network(std::io::Error),

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

    #[error("RTS error: {0}")]
    Rts(#[from] RtsError),
}

#[derive(Error, Debug)]
pub enum InitializationError {
    #[error("Logging initialization error: {0}")]
    Logging(String),

    #[error("Configuration initialization error: {0}")]
    Config(String),

    #[error("System initialization error: {kind} - {details}")]
    System {
        kind: SystemErrorKind,
        details: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },
}

#[derive(Error, Debug)]
pub enum ConnectionError {
    #[error("Connection limit exceeded: {0}")]
    LimitExceeded(String),

    #[error("Connection timed out: {0}")]
    Timeout(String),

    #[error("Invalid connection state: {0}")]
    InvalidState(String),

    #[error("Connection rejected: {0}")]
    Rejected(String),

    #[error("Backoff error: {0}")]
    Backoff(#[from] BackoffError),
}

#[derive(Error, Debug)]
pub enum BackoffError {
    #[error("Maximum retries exceeded")]
    MaxRetriesExceeded,

    #[error("Invalid backoff configuration: {0}")]
    InvalidConfig(String),

    #[error("Retry attempt failed: {0}")]
    RetryFailed(String),
}

#[derive(Error, Debug)]
pub enum ConfigValidationError {
    #[error("Invalid TCP configuration: {0}")]
    InvalidTcp(String),

    #[error("Invalid RTU configuration: {0}")]
    InvalidRtu(String),

    #[error("Invalid timing configuration: {0}")]
    InvalidTiming(String),

    #[error("Invalid security configuration: {0}")]
    InvalidSecurity(String),

    #[error("Invalid connection configuration: {0}")]
    InvalidConnection(String),
}

#[derive(Error, Debug)]
pub enum RtsError {
    #[error("Failed to set RTS signal: {0}")]
    SignalError(String),

    #[error("RTS timing error: {0}")]
    TimingError(String),

    #[error("RTS configuration error: {0}")]
    ConfigError(String),

    #[error("RTS system error: {0}")]
    SystemError(#[from] std::io::Error),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SystemErrorKind {
    ResourceAllocation,
    PermissionDenied,
    FileSystem,
    Network,
    Other,
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

#[derive(Error, Debug)]
pub enum FrameError {
    #[error("Frame size error: {kind} - {details}")]
    Size {
        kind: FrameSizeKind,
        details: String,
        frame_data: Option<Vec<u8>>,
    },

    #[error("Frame format error: {kind} - {details}")]
    Format {
        kind: FrameFormatKind,
        details: String,
        frame_data: Option<Vec<u8>>,
    },

    #[error("CRC error: calculated={calculated:04X}, received={received:04X}, frame={frame_hex}")]
    Crc {
        calculated: u16,
        received: u16,
        frame_hex: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameSizeKind {
    TooShort,
    TooLong,
    BufferOverflow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameFormatKind {
    InvalidHeader,
    InvalidFormat,
    UnexpectedResponse,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameErrorKind {
    TooShort,
    TooLong,
    InvalidFormat,
    InvalidUnitId,
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

impl InitializationError {
    pub fn logging(msg: impl Into<String>) -> Self {
        Self::Logging(msg.into())
    }

    pub fn config(msg: impl Into<String>) -> Self {
        Self::Config(msg.into())
    }

    pub fn system(kind: SystemErrorKind, details: impl Into<String>) -> Self {
        Self::System {
            kind,
            details: details.into(),
            source: None,
        }
    }

    pub fn system_with_source(
        kind: SystemErrorKind,
        details: impl Into<String>,
        source: impl Into<Box<dyn std::error::Error + Send + Sync>>,
    ) -> Self {
        Self::System {
            kind,
            details: details.into(),
            source: Some(source.into()),
        }
    }
}

impl std::fmt::Display for SystemErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ResourceAllocation => write!(f, "Resource allocation error"),
            Self::PermissionDenied => write!(f, "Permission denied"),
            Self::FileSystem => write!(f, "Filesystem error"),
            Self::Network => write!(f, "Network error"),
            Self::Other => write!(f, "Other system error"),
        }
    }
}

impl std::fmt::Display for FrameSizeKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TooShort => write!(f, "Frame too short"),
            Self::TooLong => write!(f, "Frame too long"),
            Self::BufferOverflow => write!(f, "Buffer overflow"),
        }
    }
}

impl std::fmt::Display for FrameFormatKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidHeader => write!(f, "Invalid frame header"),
            Self::InvalidFormat => write!(f, "Invalid frame format"),
            Self::UnexpectedResponse => write!(f, "Unexpected response"),
        }
    }
}

impl std::fmt::Display for FrameErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TooShort => write!(f, "Frame too short"),
            Self::TooLong => write!(f, "Frame too long"),
            Self::InvalidFormat => write!(f, "Invalid frame format"),
            Self::InvalidUnitId => write!(f, "Invalid unit ID"),
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

    pub fn connection(kind: ConnectionError) -> Self {
        RelayError::Connection(kind)
    }

    pub fn config(kind: ConfigValidationError) -> Self {
        RelayError::Config(kind)
    }

    pub fn frame(
        kind: FrameErrorKind,
        details: impl Into<String>,
        frame_data: Option<Vec<u8>>,
    ) -> Self {
        let details = details.into();
        match kind {
            FrameErrorKind::TooShort | FrameErrorKind::TooLong => {
                RelayError::Frame(FrameError::Size {
                    kind: match kind {
                        FrameErrorKind::TooShort => FrameSizeKind::TooShort,
                        FrameErrorKind::TooLong => FrameSizeKind::TooLong,
                        _ => unreachable!(),
                    },
                    details,
                    frame_data,
                })
            }
            FrameErrorKind::InvalidFormat
            | FrameErrorKind::InvalidUnitId
            | FrameErrorKind::InvalidHeader
            | FrameErrorKind::UnexpectedResponse => RelayError::Frame(FrameError::Format {
                kind: match kind {
                    FrameErrorKind::InvalidFormat => FrameFormatKind::InvalidFormat,
                    FrameErrorKind::InvalidHeader => FrameFormatKind::InvalidHeader,
                    FrameErrorKind::UnexpectedResponse => FrameFormatKind::UnexpectedResponse,
                    _ => unreachable!(),
                },
                details,
                frame_data,
            }),
            FrameErrorKind::InvalidCrc => {
                if let Some(frame_data) = frame_data {
                    let frame_hex = hex::encode(&frame_data);
                    RelayError::Frame(FrameError::Crc {
                        calculated: 0, // TODO(aljen): pass actual values
                        received: 0,   // TODO(aljen): pass actual values
                        frame_hex,
                    })
                } else {
                    RelayError::Frame(FrameError::Format {
                        kind: FrameFormatKind::InvalidFormat,
                        details,
                        frame_data: None,
                    })
                }
            }
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

impl ConnectionError {
    pub fn limit_exceeded(details: impl Into<String>) -> Self {
        ConnectionError::LimitExceeded(details.into())
    }

    pub fn timeout(details: impl Into<String>) -> Self {
        ConnectionError::Timeout(details.into())
    }

    pub fn invalid_state(details: impl Into<String>) -> Self {
        ConnectionError::InvalidState(details.into())
    }

    pub fn rejected(details: impl Into<String>) -> Self {
        ConnectionError::Rejected(details.into())
    }
}

impl ConfigValidationError {
    pub fn tcp(details: impl Into<String>) -> Self {
        ConfigValidationError::InvalidTcp(details.into())
    }

    pub fn rtu(details: impl Into<String>) -> Self {
        ConfigValidationError::InvalidRtu(details.into())
    }

    pub fn timing(details: impl Into<String>) -> Self {
        ConfigValidationError::InvalidTiming(details.into())
    }

    pub fn security(details: impl Into<String>) -> Self {
        ConfigValidationError::InvalidSecurity(details.into())
    }

    pub fn connection(details: impl Into<String>) -> Self {
        ConfigValidationError::InvalidConnection(details.into())
    }
}

impl RtsError {
    pub fn signal(details: impl Into<String>) -> Self {
        RtsError::SignalError(details.into())
    }

    pub fn timing(details: impl Into<String>) -> Self {
        RtsError::TimingError(details.into())
    }

    pub fn config(details: impl Into<String>) -> Self {
        RtsError::ConfigError(details.into())
    }
}

impl From<BackoffError> for RelayError {
    fn from(err: BackoffError) -> Self {
        RelayError::Connection(ConnectionError::Backoff(err))
    }
}

impl From<RtsError> for RelayError {
    fn from(err: RtsError) -> Self {
        RelayError::Transport(TransportError::Rts(err))
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
    fn test_connection_error_creation() {
        let err = ConnectionError::limit_exceeded("Too many connections");
        assert!(matches!(err, ConnectionError::LimitExceeded(_)));
        assert_eq!(
            err.to_string(),
            "Connection limit exceeded: Too many connections"
        );
    }

    #[test]
    fn test_config_validation_error_creation() {
        let err = ConfigValidationError::tcp("Invalid port");
        assert!(matches!(err, ConfigValidationError::InvalidTcp(_)));
        assert_eq!(err.to_string(), "Invalid TCP configuration: Invalid port");
    }

    #[test]
    fn test_rts_error_creation() {
        let err = RtsError::signal("Failed to set RTS pin");
        assert!(matches!(err, RtsError::SignalError(_)));
        assert_eq!(
            err.to_string(),
            "Failed to set RTS signal: Failed to set RTS pin"
        );
    }

    #[test]
    fn test_error_conversion() {
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080);
        let client_err = RelayError::client(ClientErrorKind::Timeout, addr, "Connection timed out");
        assert!(matches!(client_err, RelayError::Client { .. }));
    }

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
