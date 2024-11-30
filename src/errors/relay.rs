use thiserror::Error;

use super::{
    BackoffError, ClientErrorKind, ConfigValidationError, ConnectionError, FrameError,
    FrameErrorKind, FrameFormatKind, FrameSizeKind, InitializationError, ProtocolErrorKind,
    RtsError, TransportError,
};

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

impl From<config::ConfigError> for RelayError {
    fn from(err: config::ConfigError) -> Self {
        Self::Config(ConfigValidationError::config(err.to_string()))
    }
}
