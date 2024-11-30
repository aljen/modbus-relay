mod backoff_strategy;
mod guard;
mod manager;
mod stats;

pub use backoff_strategy::BackoffStrategy;
pub use guard::ConnectionGuard;
pub use manager::Manager as ConnectionManager;
pub use stats::ClientStats;
pub use stats::ConnectionStats;
pub use stats::IpStats;

#[cfg(test)]
mod tests {
    use tokio::time::sleep;

    use crate::{
        config::{BackoffConfig, ConnectionConfig},
        ConnectionError, RelayError,
    };

    use super::*;
    use std::{
        net::{IpAddr, Ipv4Addr, SocketAddr},
        sync::Arc,
        time::{Duration, Instant},
    };

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

        // First connection should succeed
        let conn1 = manager.accept_connection(addr1).await;
        assert!(conn1.is_ok(), "First connection should succeed");

        // Second connection from same IP should fail immediately (per-IP limit)
        let conn2 = manager.accept_connection(addr1).await;
        match conn2 {
            Err(RelayError::Connection(ConnectionError::LimitExceeded(msg))) => {
                assert!(
                    msg.contains("127.0.0.1:1234"),
                    "Wrong IP in error message: {}",
                    msg
                );
                return; // <-- Return here after checking error
            }
            other => panic!("Expected LimitExceeded error, got: {:?}", other),
        }
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
                active_connections: usize::MAX,
                last_active: Instant::now(),
                total_requests: u64::MAX,
                error_count: u64::MAX,
                last_error: None,
            });
            client_stats.active_connections = usize::MAX;
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
