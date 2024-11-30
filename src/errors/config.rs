use thiserror::Error;

#[derive(Error, Debug)]
pub enum ConfigValidationError {
    #[error("Configuration error: {0}")]
    Config(String),
}

impl ConfigValidationError {
    pub fn config(details: impl Into<String>) -> Self {
        Self::Config(details.into())
    }
}
