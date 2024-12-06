use std::{
    collections::HashMap,
    net::SocketAddr,
    time::{Duration, SystemTime},
};

use serde::Serialize;

use super::{ClientStats, IpStats};

#[derive(Debug, Serialize)]
pub struct Stats {
    pub total_connections: u64,
    pub active_connections: usize,
    pub total_requests: u64,
    pub total_errors: u64,
    pub requests_per_second: f64,
    pub avg_response_time_ms: u64,
    pub per_ip_stats: HashMap<SocketAddr, IpStats>,
}

impl Stats {
    pub fn from_client_stats(stats: &HashMap<SocketAddr, ClientStats>) -> Self {
        let mut total_active = 0;
        let mut total_requests = 0;
        let mut total_errors = 0;
        let mut total_response_time = 0u64;
        let mut response_time_count = 0;
        let mut per_ip = HashMap::new();

        // Calculate totals and build per-IP stats
        for (addr, client) in stats {
            total_active += client.active_connections;
            total_requests += client.total_requests;
            total_errors += client.total_errors;

            if client.avg_response_time_ms > 0 {
                total_response_time += client.avg_response_time_ms;
                response_time_count += 1;
            }

            per_ip.insert(
                *addr,
                IpStats {
                    active_connections: client.active_connections,
                    total_requests: client.total_requests,
                    total_errors: client.total_errors,
                    last_active: client.last_active,
                    last_error: client.last_error,
                    avg_response_time_ms: client.avg_response_time_ms,
                },
            );
        }

        Self {
            total_connections: total_active as u64,
            active_connections: total_active,
            total_requests,
            total_errors,
            requests_per_second: Self::calculate_requests_per_second(stats),
            avg_response_time_ms: if response_time_count > 0 {
                total_response_time / response_time_count
            } else {
                0
            },
            per_ip_stats: per_ip,
        }
    }

    fn calculate_requests_per_second(stats: &HashMap<SocketAddr, ClientStats>) -> f64 {
        let now = SystemTime::now();
        let window = Duration::from_secs(60);
        let mut recent_requests = 0;

        for client in stats.values() {
            if let Ok(duration) = now.duration_since(client.last_active) {
                if duration <= window {
                    recent_requests += client.total_requests as usize;
                }
            }
        }

        recent_requests as f64 / window.as_secs_f64()
    }
}
