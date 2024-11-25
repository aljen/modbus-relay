use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, Semaphore};

use crate::{ClientErrorKind, RelayError};

/// Configuration for managing connections
#[derive(Debug, Clone)]
pub struct ConnectionConfig {
    /// Maximum number of concurrent connections
    pub max_connections: usize,
    /// Timeout for idle connections
    pub idle_timeout: Duration,
    /// Timeout for establishing a connection
    pub connect_timeout: Duration,
    /// Limits for specific IP addresses
    pub per_ip_limits: Option<usize>,
    /// Parameters for backoff strategy
    pub backoff: BackoffConfig,
}

#[derive(Debug, Clone)]
pub struct BackoffConfig {
    /// Initial wait time
    pub initial_interval: Duration,
    /// Maximum wait time
    pub max_interval: Duration,
    /// Multiplier for each subsequent attempt
    pub multiplier: f64,
    /// Maximum number of attempts
    pub max_retries: usize,
}

impl Default for ConnectionConfig {
    fn default() -> Self {
        Self {
            max_connections: 100,
            idle_timeout: Duration::from_secs(60),
            connect_timeout: Duration::from_secs(5),
            per_ip_limits: Some(10),
            backoff: BackoffConfig {
                initial_interval: Duration::from_millis(100),
                max_interval: Duration::from_secs(30),
                multiplier: 2.0,
                max_retries: 5,
            },
        }
    }
}

/// Stats for a single client
#[derive(Debug)]
struct ClientStats {
    /// Number of active connections from this address
    active_connections: usize,
    /// Last activity
    last_active: Instant,
    /// Total number of requests
    total_requests: u64,
    /// Number of errors
    error_count: u64,
    /// Timestamp of the last error
    last_error: Option<Instant>,
}

/// TCP connection management
pub struct ConnectionManager {
    /// Connection limit per IP
    per_ip_semaphores: Arc<Mutex<HashMap<SocketAddr, Arc<Semaphore>>>>,
    /// Global connection limit
    global_semaphore: Arc<Semaphore>,
    /// Stats per IP
    stats: Arc<Mutex<HashMap<SocketAddr, ClientStats>>>,
    /// Configuration
    config: ConnectionConfig,
    /// Counter of all connections
    total_connections: Arc<AtomicU64>,
}

impl ConnectionManager {
    pub fn new(config: ConnectionConfig) -> Self {
        Self {
            per_ip_semaphores: Arc::new(Mutex::new(HashMap::new())),
            global_semaphore: Arc::new(Semaphore::new(config.max_connections)),
            stats: Arc::new(Mutex::new(HashMap::new())),
            config,
            total_connections: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Attempt to establish a new connection
    pub async fn accept_connection(
        self: &Arc<Self>,
        addr: SocketAddr,
    ) -> Result<ConnectionGuard, RelayError> {
        // Check if the global limit has been exceeded
        let global_permit = self
            .global_semaphore
            .clone()
            .acquire_owned()
            .await
            .map_err(|_| {
                RelayError::client(
                    ClientErrorKind::TooManyConnections,
                    addr,
                    "Global connection limit reached",
                )
            })?;

        // Check per IP limit if enabled
        if let Some(per_ip_limit) = self.config.per_ip_limits {
            let mut semaphores = self.per_ip_semaphores.lock().await;
            let semaphore = semaphores
                .entry(addr)
                .or_insert_with(|| Arc::new(Semaphore::new(per_ip_limit)));

            let _ = semaphore.try_acquire().map_err(|_| {
                RelayError::client(
                    ClientErrorKind::TooManyConnections,
                    addr,
                    format!("Per-IP limit ({}) reached", per_ip_limit),
                )
            })?;
        }

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

            client_stats.active_connections += 1;
            client_stats.last_active = Instant::now();
        }

        self.total_connections.fetch_add(1, Ordering::Relaxed);

        Ok(ConnectionGuard {
            manager: Arc::clone(self),
            addr,
            _global_permit: global_permit,
        })
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

    /// Cleans up idle connections
    pub async fn cleanup_idle_connections(&self) {
        let now = Instant::now();
        let mut stats = self.stats.lock().await;
        stats.retain(|_, s| now.duration_since(s.last_active) < self.config.idle_timeout);
    }

    /// Returns connection statistics
    pub async fn get_stats(&self) -> ConnectionStats {
        let stats = self.stats.lock().await;
        let mut total_active = 0;
        let mut total_requests = 0;
        let mut total_errors = 0;
        let mut per_ip_stats = HashMap::new();

        for (addr, client_stats) in stats.iter() {
            total_active += client_stats.active_connections;
            total_requests += client_stats.total_requests;
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

        ConnectionStats {
            total_connections: self.total_connections.load(Ordering::Relaxed),
            active_connections: total_active,
            total_requests,
            total_errors,
            per_ip_stats,
        }
    }
}

/// RAII guard for the connection
pub struct ConnectionGuard {
    manager: Arc<ConnectionManager>,
    addr: SocketAddr,
    _global_permit: tokio::sync::OwnedSemaphorePermit,
}

#[derive(Debug)]
pub struct ConnectionStats {
    pub total_connections: u64,
    pub active_connections: usize,
    pub total_requests: u64,
    pub total_errors: u64,
    pub per_ip_stats: HashMap<SocketAddr, IpStats>,
}

#[derive(Debug)]
pub struct IpStats {
    pub active_connections: usize,
    pub total_requests: u64,
    pub error_count: u64,
    pub last_active: Instant,
    pub last_error: Option<Instant>,
}

// Helper for implementing backoff strategy
pub struct BackoffStrategy {
    config: BackoffConfig,
    current_attempt: usize,
    last_attempt: Option<Instant>,
}

impl Drop for ConnectionGuard {
    fn drop(&mut self) {
        let manager = Arc::clone(&self.manager);
        let addr = self.addr;

        tokio::spawn(async move {
            let mut stats = manager.stats.lock().await;
            if let Some(client_stats) = stats.get_mut(&addr) {
                client_stats.active_connections -= 1;
            }
        });
    }
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
        if self.current_attempt >= self.config.max_retries {
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

// Testy
#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr};

    #[tokio::test]
    async fn test_connection_limits() {
        let config = ConnectionConfig {
            max_connections: 2,
            per_ip_limits: Some(1),
            ..Default::default()
        };

        let manager = Arc::new(ConnectionManager::new(config));
        let addr1 = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 1234);
        let addr2 = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 2)), 1234);

        // First connection should succeed
        let conn1 = manager.accept_connection(addr1).await;
        assert!(conn1.is_ok());

        // Second connection from the same IP should fail
        let conn2 = manager.accept_connection(addr1).await;
        assert!(
            conn2.is_err(),
            "Expected connection limit error for same IP"
        );

        // Connection from different IP should succeed
        let conn3 = manager.accept_connection(addr2).await;
        assert!(conn3.is_ok());

        // Third connection should fail (global limit)
        let conn4 = manager.accept_connection(addr2).await;
        assert!(conn4.is_err(), "Expected global connection limit error");
    }

    #[tokio::test]
    async fn test_backoff_strategy() {
        let config = BackoffConfig {
            initial_interval: Duration::from_millis(100),
            max_interval: Duration::from_secs(1),
            multiplier: 2.0,
            max_retries: 3,
        };

        let mut strategy = BackoffStrategy::new(config);

        // The first attempts should return increasing values
        assert_eq!(strategy.next_backoff().unwrap().as_millis(), 100);
        assert_eq!(strategy.next_backoff().unwrap().as_millis(), 200);
        assert_eq!(strategy.next_backoff().unwrap().as_millis(), 400);

        // After exhausting attempts, it should return None
        assert!(strategy.next_backoff().is_none());

        // After reset, it should start from the beginning
        strategy.reset();
        assert_eq!(strategy.next_backoff().unwrap().as_millis(), 100);
    }
}
