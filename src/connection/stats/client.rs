use std::time::SystemTime;

use serde::Serialize;

/// Stats for a single client
#[derive(Debug, Clone, Serialize)]
pub struct Stats {
    /// Number of active connections from this address
    pub active_connections: usize,
    /// Total number of requests
    pub total_requests: u64,
    /// Total number of errors
    pub total_errors: u64,
    /// Last activity
    pub last_active: SystemTime,
    /// Timestamp of the last error
    pub last_error: Option<SystemTime>,
    /// Average response time
    pub avg_response_time_ms: u64,
}

impl Default for Stats {
    fn default() -> Self {
        Self {
            active_connections: 0,
            total_requests: 0,
            total_errors: 0,
            last_active: SystemTime::now(),
            last_error: None,
            avg_response_time_ms: 0,
        }
    }
}
