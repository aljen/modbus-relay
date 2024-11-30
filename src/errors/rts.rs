use thiserror::Error;

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
