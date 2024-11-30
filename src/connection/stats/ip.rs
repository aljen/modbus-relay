use std::time::Instant;

#[derive(Debug)]
pub struct Stats {
    pub active_connections: usize,
    pub total_requests: u64,
    pub error_count: u64,
    pub last_active: Instant,
    pub last_error: Option<Instant>,
}
