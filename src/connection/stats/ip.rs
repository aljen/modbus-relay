use std::time::SystemTime;

use serde::Serialize;

/// Stats for a single IP address
#[derive(Debug, Clone, Serialize)]
pub struct Stats {
    pub active_connections: usize,
    pub total_requests: u64,
    pub total_errors: u64,
    pub last_active: SystemTime,
    pub last_error: Option<SystemTime>,
    pub avg_response_time_ms: u64,
}
