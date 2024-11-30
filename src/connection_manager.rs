use std::collections::{HashMap, VecDeque};
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::{Mutex, Semaphore};
use tracing::info;

use crate::{ConfigValidationError, ConnectionError, RelayError};

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

impl Default for BackoffConfig {
    fn default() -> Self {
        Self {
            initial_interval: Duration::from_millis(100),
            max_interval: Duration::from_secs(30),
            multiplier: 2.0,
            max_retries: 5,
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
#[derive(Debug)]
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
    /// Total requests
    total_requests: AtomicU64,
    /// Error count
    error_count: AtomicU32,
    /// Start time
    start_time: Instant,
    /// Response times
    response_times: Arc<Mutex<VecDeque<Duration>>>,
}

impl ConnectionConfig {
    pub fn validate(&self) -> Result<(), ConfigValidationError> {
        if self.max_connections == 0 {
            return Err(ConfigValidationError::connection(
                "max_connections cannot be 0".to_string(),
            ));
        }

        if let Some(limit) = self.per_ip_limits {
            if limit == 0 {
                return Err(ConfigValidationError::connection(
                    "per_ip_limits cannot be 0".to_string(),
                ));
            }
            if limit > self.max_connections {
                return Err(ConfigValidationError::connection(format!(
                    "per_ip_limits ({}) cannot be greater than max_connections ({})",
                    limit, self.max_connections
                )));
            }
        }

        if self.idle_timeout.as_secs() == 0 {
            return Err(ConfigValidationError::connection(
                "idle_timeout cannot be 0".to_string(),
            ));
        }

        if self.connect_timeout.as_secs() == 0 {
            return Err(ConfigValidationError::connection(
                "connect_timeout cannot be 0".to_string(),
            ));
        }

        Ok(())
    }
}

impl ConnectionManager {
    pub fn new(config: ConnectionConfig) -> Self {
        Self {
            per_ip_semaphores: Arc::new(Mutex::new(HashMap::new())),
            global_semaphore: Arc::new(Semaphore::new(config.max_connections)),
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

        // Check per IP limit if enabled
        if let Some(per_ip_limit) = self.config.per_ip_limits {
            let mut semaphores = self.per_ip_semaphores.lock().await;
            let semaphore = semaphores
                .entry(addr)
                .or_insert_with(|| Arc::new(Semaphore::new(per_ip_limit)));

            let _ = semaphore.try_acquire().map_err(|_| {
                RelayError::Connection(ConnectionError::limit_exceeded(format!(
                    "Per-IP limit ({}) reached for {}",
                    per_ip_limit, addr
                )))
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

    /// Cleans up idle connections
    pub async fn cleanup_idle_connections(&self) -> Result<(), RelayError> {
        let now = Instant::now();
        let mut stats = self.stats.lock().await;
        let mut cleanup_errors = Vec::new();

        stats.retain(|addr, s| {
            if now.duration_since(s.last_active) >= self.config.idle_timeout {
                if s.active_connections > 0 {
                    cleanup_errors.push(format!(
                        "Forced cleanup of {} active connections for {}",
                        s.active_connections, addr
                    ));
                }
                false
            } else {
                true
            }
        });

        if !cleanup_errors.is_empty() {
            return Err(RelayError::Connection(ConnectionError::invalid_state(
                format!("Cleanup encountered issues: {}", cleanup_errors.join(", ")),
            )));
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

/// RAII guard for the connection
#[derive(Debug)]
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
    use tokio::time::sleep;

    use super::*;
    use std::net::{IpAddr, Ipv4Addr};

    #[tokio::test]
    async fn test_connection_limits() {
        let config = ConnectionConfig {
            max_connections: 2,
            per_ip_limits: Some(1),
            idle_timeout: Duration::from_secs(60),
            connect_timeout: Duration::from_secs(5),
            backoff: BackoffConfig::default(),
        };

        let manager = Arc::new(ConnectionManager::new(config));
        let addr1 = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 1234);
        let addr2 = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 2)), 1234);

        // First connection should succeed
        let conn1 = manager.accept_connection(addr1).await;
        assert!(conn1.is_ok(), "First connection should succeed");

        // Second connection from same IP should fail immediately (per-IP limit)
        let conn2 = manager.accept_connection(addr1).await;
        match conn2 {
            Err(RelayError::Connection(ConnectionError::LimitExceeded(msg))) => {
                assert!(
                    msg.contains("Per-IP limit (1) reached"),
                    "Wrong error message: {}",
                    msg
                );
            }
            other => panic!("Expected LimitExceeded error, got: {:?}", other),
        }

        // Connection from different IP should succeed
        let conn3 = manager.accept_connection(addr2).await;
        assert!(conn3.is_ok(), "Connection from different IP should succeed");

        // Third connection should fail immediately (global limit)
        let conn4 = manager.accept_connection(addr2).await;
        match conn4 {
            Err(RelayError::Connection(ConnectionError::LimitExceeded(msg))) => {
                assert!(
                    msg.contains("Global connection limit"),
                    "Wrong error message: {}",
                    msg
                );
            }
            other => panic!("Expected LimitExceeded error, got: {:?}", other),
        }

        // Drop first connection and try again - should succeed
        drop(conn1);
        tokio::time::sleep(Duration::from_millis(10)).await; // Give time for cleanup

        let conn5 = manager.accept_connection(addr1).await;
        assert!(conn5.is_ok(), "Connection after drop should succeed");
    }

    #[tokio::test]
    async fn test_connection_stats_after_limit() {
        let config = ConnectionConfig {
            max_connections: 1,
            per_ip_limits: Some(1),
            ..Default::default()
        };

        let manager = Arc::new(ConnectionManager::new(config));
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 1234);

        // First connection succeeds
        let conn = manager.accept_connection(addr).await.unwrap();

        // Second connection fails
        let _err = manager.accept_connection(addr).await.unwrap_err();

        // Check stats
        let stats = manager.get_stats().await.unwrap(); // Unwrap Result
        assert_eq!(
            stats.active_connections, 1,
            "Should have one active connection"
        );
        assert_eq!(
            stats.total_connections, 1,
            "Should have one total connection"
        );

        // Cleanup
        drop(conn);
    }

    #[tokio::test]
    async fn test_error_recording() {
        let manager = Arc::new(ConnectionManager::new(ConnectionConfig::default()));
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 1234);

        // Record some errors
        assert!(manager.record_client_error(&addr).await.is_ok());

        // Verify error was recorded
        let stats = manager.get_stats().await.unwrap();
        assert_eq!(stats.total_errors, 1);
        assert!(stats.per_ip_stats.get(&addr).unwrap().error_count == 1);
    }

    #[tokio::test]
    async fn test_idle_connection_cleanup() {
        let config = ConnectionConfig {
            idle_timeout: Duration::from_millis(100),
            ..Default::default()
        };

        let manager = Arc::new(ConnectionManager::new(config));
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 1234);

        // Create a connection
        let _conn = manager.accept_connection(addr).await.unwrap();

        // Verify connection is active
        let stats = manager.get_stats().await.unwrap();
        assert_eq!(stats.active_connections, 1);

        // Wait for connection to become idle
        sleep(Duration::from_millis(200)).await;

        // Cleanup should work
        assert!(manager.cleanup_idle_connections().await.is_ok());

        // Verify connection was cleaned up
        let stats = manager.get_stats().await.unwrap();
        assert_eq!(stats.active_connections, 0);
    }

    #[tokio::test]
    async fn test_stats_counter_overflow() {
        let manager = Arc::new(ConnectionManager::new(ConnectionConfig::default()));
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 1234);

        // Manually set counters to near max to test overflow protection
        {
            let mut stats = manager.stats.lock().await;
            let client_stats = stats.entry(addr).or_insert_with(|| ClientStats {
                active_connections: usize::MAX - 1,
                last_active: Instant::now(),
                total_requests: u64::MAX - 1,
                error_count: u64::MAX - 1,
                last_error: None,
            });
            client_stats.active_connections = usize::MAX - 1;
        }

        // Attempting to increment should result in error
        let result = manager.accept_connection(addr).await;
        assert!(matches!(
            result.unwrap_err(),
            RelayError::Connection(ConnectionError::InvalidState(_))
        ));
    }

    #[tokio::test]
    async fn test_connection_guard_cleanup() {
        let manager = Arc::new(ConnectionManager::new(ConnectionConfig::default()));
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 1234);

        {
            let guard = manager.accept_connection(addr).await.unwrap();
            let stats = manager.get_stats().await.unwrap();
            assert_eq!(stats.active_connections, 1);

            // Guard should clean up when dropped
            drop(guard);
        }

        // Wait a bit for async cleanup
        sleep(Duration::from_millis(50)).await;

        let stats = manager.get_stats().await.unwrap();
        assert_eq!(stats.active_connections, 0);
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
