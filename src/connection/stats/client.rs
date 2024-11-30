use std::time::Instant;

/// Stats for a single client
#[derive(Debug)]
pub struct Stats {
    /// Number of active connections from this address
    pub active_connections: usize,
    /// Last activity
    pub last_active: Instant,
    /// Total number of requests
    pub total_requests: u64,
    /// Number of errors
    pub error_count: u64,
    /// Timestamp of the last error
    pub last_error: Option<Instant>,
}
