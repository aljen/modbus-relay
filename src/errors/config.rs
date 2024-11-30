use thiserror::Error;

#[derive(Error, Debug)]
pub enum ConfigValidationError {
    #[error("TCP configuration error: {0}")]
    Tcp(String),

    #[error("RTU configuration error: {0}")]
    Rtu(String),

    #[error("Timing configuration error: {0}")]
    Timing(String),

    #[error("Security configuration error: {0}")]
    Security(String),

    #[error("Connection configuration error: {0}")]
    Connection(String),

    #[error("Configuration error: {0}")]
    Config(String),
}

impl ConfigValidationError {
    pub fn tcp(details: impl Into<String>) -> Self {
        Self::Tcp(details.into())
    }

    pub fn rtu(details: impl Into<String>) -> Self {
        Self::Rtu(details.into())
    }

    pub fn timing(details: impl Into<String>) -> Self {
        Self::Timing(details.into())
    }

    pub fn security(details: impl Into<String>) -> Self {
        Self::Security(details.into())
    }

    pub fn connection(details: impl Into<String>) -> Self {
        Self::Connection(details.into())
    }

    pub fn config(details: impl Into<String>) -> Self {
        Self::Config(details.into())
    }
}
