use thiserror::Error;

#[derive(Error, Debug)]
pub enum BackoffError {
    #[error("Maximum retries exceeded")]
    MaxRetriesExceeded,

    #[error("Invalid backoff configuration: {0}")]
    InvalidConfig(String),

    #[error("Retry attempt failed: {0}")]
    RetryFailed(String),
}
