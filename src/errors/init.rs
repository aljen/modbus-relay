use thiserror::Error;

use super::SystemErrorKind;

#[derive(Error, Debug)]
pub enum InitializationError {
    #[error("Invalid log level: {0}")]
    InvalidLogLevel(String),

    #[error("Invalid log format: {0}")]
    InvalidLogFormat(String),

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
