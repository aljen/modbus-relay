use std::time::{Duration, Instant};

use crate::config::BackoffConfig;

/// Helper for implementing backoff strategy
pub struct BackoffStrategy {
    config: BackoffConfig,
    current_attempt: usize,
    last_attempt: Option<Instant>,
}

impl BackoffStrategy {
    pub fn new(config: BackoffConfig) -> Self {
        Self {
            config,
            current_attempt: 0,
            last_attempt: None,
        }
    }

    pub fn next_backoff(&mut self) -> Option<Duration> {
        if self.current_attempt >= self.config.max_retries as usize {
            return None;
        }

        let interval = self.config.initial_interval.as_secs_f64()
            * self.config.multiplier.powi(self.current_attempt as i32);

        let interval =
            Duration::from_secs_f64(interval.min(self.config.max_interval.as_secs_f64()));

        self.current_attempt += 1;
        self.last_attempt = Some(Instant::now());
        Some(interval)
    }

    pub fn reset(&mut self) {
        self.current_attempt = 0;
        self.last_attempt = None;
    }
}
