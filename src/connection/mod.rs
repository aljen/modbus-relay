mod backoff_strategy;
mod events;
mod guard;
mod manager;
mod stats;

pub use backoff_strategy::BackoffStrategy;
pub use events::StatEvent;
pub use guard::ConnectionGuard;
pub use manager::Manager as ConnectionManager;
pub use stats::ClientStats;
pub use stats::ConnectionStats;
pub use stats::IpStats;

#[cfg(test)]
mod tests {
    use tokio::{
        sync::{broadcast, mpsc, Mutex},
        time::sleep,
    };

    use crate::{
        config::{BackoffConfig, ConnectionConfig},
        ConnectionError, RelayError, StatsConfig, StatsManager,
    };

    use super::*;
    use std::{
        collections::HashMap,
        net::{IpAddr, Ipv4Addr, SocketAddr},
        sync::Arc,
        time::Duration,
    };

    #[tokio::test]
    async fn test_connection_limits() {
        let config = ConnectionConfig {
            max_connections: 2,
            per_ip_limits: Some(1),
            idle_timeout: Duration::from_secs(60),
            error_timeout: Duration::from_secs(300),
            connect_timeout: Duration::from_secs(5),
            backoff: BackoffConfig::default(),
        };

        let (stats_tx, _) = mpsc::channel(100);
        let manager = Arc::new(ConnectionManager::new(config, stats_tx));
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

        let stats_config = StatsConfig::default();

        let (stats_manager, stats_tx) = StatsManager::new(stats_config);
        let stats_manager = Arc::new(Mutex::new(stats_manager));

        let (shutdown_tx, shutdown_rx) = broadcast::channel(1);

        let stats_handle = tokio::spawn({
            async move {
                let mut stats_manager = stats_manager.lock().await;
                stats_manager.run(shutdown_rx).await;
            }
        });

        let manager = Arc::new(ConnectionManager::new(config, stats_tx));

        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 1234);

        // First connection succeeds
        let conn = manager.accept_connection(addr).await.unwrap();

        // Second connection fails
        let _err = manager.accept_connection(addr).await.unwrap_err();

        // Check stats
        let stats = manager.get_stats().await.unwrap();

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

        shutdown_tx.send(()).unwrap();
        stats_handle.await.unwrap();
    }

    #[tokio::test]
    async fn test_idle_connection_cleanup() {
        let config = ConnectionConfig {
            idle_timeout: Duration::from_millis(100),
            ..Default::default()
        };

        let stats_config = StatsConfig {
            cleanup_interval: config.idle_timeout,
            idle_timeout: config.idle_timeout,
            error_timeout: config.error_timeout,
            max_events_per_second: 10000, // TODO(aljen): Make configurable
        };

        let (stats_manager, stats_tx) = StatsManager::new(stats_config);
        let stats_manager = Arc::new(Mutex::new(stats_manager));

        let (shutdown_tx, shutdown_rx) = broadcast::channel(1);

        let stats_handle = tokio::spawn({
            async move {
                let mut stats_manager = stats_manager.lock().await;
                stats_manager.run(shutdown_rx).await;
            }
        });

        let manager = Arc::new(ConnectionManager::new(config, stats_tx));
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

        shutdown_tx.send(()).unwrap();
        stats_handle.await.unwrap();
    }

    #[tokio::test]
    async fn test_connection_guard_cleanup() {
        let config = ConnectionConfig::default();

        let stats_config = StatsConfig {
            cleanup_interval: config.idle_timeout,
            idle_timeout: config.idle_timeout,
            error_timeout: config.error_timeout,
            max_events_per_second: 10000, // TODO(aljen): Make configurable
        };

        let (stats_manager, stats_tx) = StatsManager::new(stats_config);
        let stats_manager = Arc::new(Mutex::new(stats_manager));

        let (shutdown_tx, shutdown_rx) = broadcast::channel(1);

        let stats_handle = tokio::spawn({
            async move {
                let mut stats_manager = stats_manager.lock().await;
                stats_manager.run(shutdown_rx).await;
            }
        });

        let manager = Arc::new(ConnectionManager::new(config, stats_tx));

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

        shutdown_tx.send(()).unwrap();
        stats_handle.await.unwrap();
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

    #[tokio::test]
    async fn test_connection_lifecycle() {
        let config = ConnectionConfig::default();
        let (stats_tx, mut stats_rx) = mpsc::channel(100);
        let manager = Arc::new(ConnectionManager::new(config, stats_tx));

        // Handle stats events in background
        tokio::spawn(async move {
            while let Some(event) = stats_rx.recv().await {
                match event {
                    StatEvent::QueryConnectionStats { response_tx } => {
                        let _ = response_tx.send(ConnectionStats {
                            total_connections: 1,
                            active_connections: 1,
                            total_requests: 0,
                            total_errors: 0,
                            requests_per_second: 0.0,
                            avg_response_time_ms: 0,
                            per_ip_stats: HashMap::new(),
                        });
                    }
                    _ => {}
                }
            }
        });

        let addr = "127.0.0.1:8080".parse().unwrap();

        // Test connection acceptance
        let guard = manager.accept_connection(addr).await.unwrap();
        assert_eq!(manager.get_connection_count(&addr).await, 1);

        // Test statistics
        let stats = manager.get_stats().await.unwrap();
        assert_eq!(stats.active_connections, 1);

        // Test connection cleanup
        drop(guard);
        sleep(Duration::from_millis(100)).await;
        assert_eq!(manager.get_connection_count(&addr).await, 0);
    }
}
