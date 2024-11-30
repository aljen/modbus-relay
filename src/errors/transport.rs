use std::time::Duration;
use thiserror::Error;
use tokio::time::error::Elapsed;

use super::{IoOperation, RtsError, SerialErrorKind};

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

    #[error("I/O error: {operation} failed on {details}")]
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
