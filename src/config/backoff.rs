use std::time::Duration;

#[derive(Debug, Clone)]
pub struct Config {
    /// Initial wait time
    pub initial_interval: Duration,
    /// Maximum wait time
    pub max_interval: Duration,
    /// Multiplier for each subsequent attempt
    pub multiplier: f64,
    /// Maximum number of attempts
    pub max_retries: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            initial_interval: Duration::from_millis(100),
            max_interval: Duration::from_secs(30),
            multiplier: 2.0,
            max_retries: 5,
        }
    }
}
