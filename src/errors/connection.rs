use thiserror::Error;

use super::BackoffError;

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

    #[error("Connection disconnected")]
    Disconnected,

    #[error("Backoff error: {0}")]
    Backoff(#[from] BackoffError),
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
