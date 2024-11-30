use std::{collections::HashMap, net::SocketAddr};

use super::IpStats;

#[derive(Debug)]
pub struct Stats {
    pub total_connections: u64,
    pub active_connections: usize,
    pub total_requests: u64,
    pub total_errors: u64,
    pub per_ip_stats: HashMap<SocketAddr, IpStats>,
}
