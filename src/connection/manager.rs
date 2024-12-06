use std::{
    collections::{HashMap, VecDeque},
    net::SocketAddr,
    sync::{
        atomic::{AtomicU32, AtomicU64, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

use tokio::sync::{Mutex, Semaphore};
use tracing::info;

use crate::{config::ConnectionConfig, ConnectionError, RelayError};

use super::{ClientStats, ConnectionGuard, ConnectionStats, IpStats};

/// TCP connection management
#[derive(Debug)]
pub struct Manager {
    /// Connection limit per IP
    pub per_ip_semaphores: Arc<Mutex<HashMap<SocketAddr, Arc<Semaphore>>>>,
    /// Global connection limit
    pub global_semaphore: Arc<Semaphore>,
    /// Stats per IP
    pub stats: Arc<Mutex<HashMap<SocketAddr, ClientStats>>>,
    /// Configuration
    pub config: ConnectionConfig,
    /// Counter of all connections
    pub total_connections: Arc<AtomicU64>,
    /// Total requests
    pub total_requests: AtomicU64,
    /// Error count
    pub error_count: AtomicU32,
    /// Start time
    pub start_time: Instant,
    /// Response times
    pub response_times: Arc<Mutex<VecDeque<Duration>>>,
}

impl Manager {
    pub fn new(config: ConnectionConfig) -> Self {
        Self {
            per_ip_semaphores: Arc::new(Mutex::new(HashMap::new())),
            global_semaphore: Arc::new(Semaphore::new(config.max_connections as usize)),
            stats: Arc::new(Mutex::new(HashMap::new())),
            config,
            total_connections: Arc::new(AtomicU64::new(0)),
            total_requests: AtomicU64::new(0),
            error_count: AtomicU32::new(0),
            start_time: Instant::now(),
            response_times: Arc::new(Mutex::new(VecDeque::with_capacity(100))),
        }
    }

    /// Attempt to establish a new connection
    pub async fn accept_connection(
        self: &Arc<Self>,
        addr: SocketAddr,
    ) -> Result<ConnectionGuard, RelayError> {
        // Check per IP limit if enabled
        let per_ip_permit = if let Some(per_ip_limit) = self.config.per_ip_limits {
            let mut semaphores = self.per_ip_semaphores.lock().await;

            let semaphore = semaphores
                .entry(addr)
                .or_insert_with(|| Arc::new(Semaphore::new(per_ip_limit as usize)));

            Some(semaphore.clone().try_acquire_owned().map_err(|_| {
                RelayError::Connection(ConnectionError::limit_exceeded(format!(
                    "Per-IP limit ({}) reached for {}",
                    per_ip_limit, addr
                )))
            })?)
        } else {
            None
        };

        // Check if the global limit has been exceeded
        let global_permit = self
            .global_semaphore
            .clone()
            .try_acquire_owned()
            .map_err(|_| {
                RelayError::Connection(ConnectionError::limit_exceeded(
                    "Global connection limit reached",
                ))
            })?;

        // Update statistics
        {
            let mut stats = self.stats.lock().await;
            let client_stats = stats.entry(addr).or_insert_with(|| ClientStats {
                active_connections: 0,
                last_active: Instant::now(),
                total_requests: 0,
                error_count: 0,
                last_error: None,
            });

            // Check for potential overflow
            if client_stats.active_connections == usize::MAX {
                return Err(RelayError::Connection(ConnectionError::invalid_state(
                    "Active connections counter overflow".to_string(),
                )));
            }

            client_stats.active_connections += 1;
            client_stats.last_active = Instant::now();
        }

        self.total_connections.fetch_add(1, Ordering::Relaxed);

        Ok(ConnectionGuard {
            manager: Arc::clone(self),
            addr,
            _global_permit: global_permit,
            _per_ip_permit: per_ip_permit,
        })
    }

    pub async fn close_all_connections(&self) -> Result<(), RelayError> {
        let stats = self.stats.lock().await;
        let active_connections = stats.values().map(|s| s.active_connections).sum::<usize>();

        if active_connections > 0 {
            info!("Closing {} active connections", active_connections);
            // TODO(aljen): Here we can add code to forcefully close connections
            // e.g., by sending a signal to all ConnectionGuard
        }

        Ok(())
    }

    pub async fn record_client_error(&self, addr: &SocketAddr) -> Result<(), RelayError> {
        let mut stats = self.stats.lock().await;
        let client_stats = stats.entry(*addr).or_insert_with(|| ClientStats {
            active_connections: 0,
            last_active: Instant::now(),
            total_requests: 0,
            error_count: 0,
            last_error: None,
        });

        client_stats.error_count += 1;
        client_stats.last_error = Some(Instant::now());

        Ok(())
    }

    /// Updates statistics for a given connection
    pub async fn record_request(&self, addr: SocketAddr, success: bool) {
        let mut stats = self.stats.lock().await;
        if let Some(client_stats) = stats.get_mut(&addr) {
            client_stats.total_requests += 1;
            client_stats.last_active = Instant::now();
            if !success {
                client_stats.error_count += 1;
                client_stats.last_error = Some(Instant::now());
            }
        }
    }

    fn should_cleanup_connection(
        stats: &ClientStats,
        now: Instant,
        idle_timeout: Duration,
        error_timeout: Duration,
    ) -> bool {
        now.duration_since(stats.last_active) >= idle_timeout
            || (stats.error_count > 0
                && now.duration_since(stats.last_error.unwrap_or(now)) >= error_timeout)
    }

    /// Cleans up idle connections
    pub async fn cleanup_idle_connections(&self) -> Result<(), RelayError> {
        let now = Instant::now();

        // First pass: collect connections to clean
        let to_clean: Vec<(SocketAddr, ClientStats)> = {
            let stats = self.stats.lock().await;
            stats
                .iter()
                .filter(|(_, s)| {
                    Self::should_cleanup_connection(
                        s,
                        now,
                        self.config.idle_timeout,
                        self.config.error_timeout,
                    )
                })
                .map(|(addr, s)| (*addr, (*s).clone()))
                .collect()
        }; // stats lock is dropped here

        // Second pass: verify and cleanup
        for (addr, stats_snapshot) in to_clean {
            let mut stats = self.stats.lock().await;
            // Recheck conditions before cleanup
            if Self::should_cleanup_connection(
                &stats_snapshot,
                now,
                self.config.idle_timeout,
                self.config.error_timeout,
            ) {
                stats.remove(&addr);
                info!(
                    "Cleaned up connection {} ({} connections, {} errors, last active: {:?} ago)",
                    addr,
                    stats_snapshot.active_connections,
                    stats_snapshot.error_count,
                    now.duration_since(stats_snapshot.last_active)
                );
            }
        }

        Ok(())
    }

    /// Returns connection statistics
    pub async fn get_stats(&self) -> Result<ConnectionStats, RelayError> {
        let stats = self.stats.lock().await;
        let mut total_active: usize = 0;
        let mut total_requests: u64 = 0;
        let mut total_errors: u64 = 0;
        let mut per_ip_stats = HashMap::new();

        for (addr, client_stats) in stats.iter() {
            // Check for counter overflow
            if total_active
                .checked_add(client_stats.active_connections)
                .is_none()
            {
                return Err(RelayError::Connection(ConnectionError::invalid_state(
                    "Total active connections counter overflow".to_string(),
                )));
            }
            total_active += client_stats.active_connections;

            if total_requests
                .checked_add(client_stats.total_requests)
                .is_none()
            {
                return Err(RelayError::Connection(ConnectionError::invalid_state(
                    "Total requests counter overflow".to_string(),
                )));
            }
            total_requests += client_stats.total_requests;

            if total_errors.checked_add(client_stats.error_count).is_none() {
                return Err(RelayError::Connection(ConnectionError::invalid_state(
                    "Total errors counter overflow".to_string(),
                )));
            }
            total_errors += client_stats.error_count;

            per_ip_stats.insert(
                *addr,
                IpStats {
                    active_connections: client_stats.active_connections,
                    total_requests: client_stats.total_requests,
                    error_count: client_stats.error_count,
                    last_active: client_stats.last_active,
                    last_error: client_stats.last_error,
                },
            );
        }

        Ok(ConnectionStats {
            total_connections: self.total_connections.load(Ordering::Relaxed),
            active_connections: total_active,
            total_requests,
            total_errors,
            per_ip_stats,
        })
    }

    pub async fn connection_count(&self) -> u32 {
        self.stats.lock().await.len() as u32
    }

    pub fn total_requests(&self) -> u64 {
        self.total_requests.load(Ordering::Relaxed)
    }

    pub fn error_count(&self) -> u32 {
        self.error_count.load(Ordering::Relaxed)
    }

    pub async fn avg_response_time(&self) -> Duration {
        let times = self.response_times.lock().await;
        if times.is_empty() {
            return Duration::from_millis(0);
        }
        let sum: Duration = times.iter().sum();
        sum / times.len() as u32
    }

    pub fn requests_per_second(&self) -> f64 {
        let total = self.total_requests.load(Ordering::Relaxed) as f64;
        let elapsed = self.start_time.elapsed().as_secs_f64();
        if elapsed > 0.0 {
            total / elapsed
        } else {
            0.0
        }
    }

    pub fn record_requests(&self) {
        self.total_requests.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_errors(&self) {
        self.error_count.fetch_add(1, Ordering::Relaxed);
    }

    pub async fn record_response_time(&self, duration: Duration) {
        let mut times = self.response_times.lock().await;
        if times.len() >= 100 {
            times.pop_front();
        }
        times.push_back(duration);
    }
}
