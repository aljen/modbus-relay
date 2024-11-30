use thiserror::Error;

#[derive(Error, Debug)]
pub enum InitializationError {
    #[error("Logging initialization error: {0}")]
    Logging(String),
}

impl InitializationError {
    pub fn logging(msg: impl Into<String>) -> Self {
        Self::Logging(msg.into())
    }
}
