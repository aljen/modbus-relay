use std::{collections::HashMap, net::SocketAddr, sync::Arc, time::SystemTime};

use tokio::sync::{Mutex, mpsc};
use tracing::{debug, info, warn};

use crate::{ClientStats, ConnectionStats, config::StatsConfig, connection::StatEvent};

pub struct StatsManager {
    stats: Arc<Mutex<HashMap<SocketAddr, ClientStats>>>,
    event_rx: mpsc::Receiver<StatEvent>,
    config: StatsConfig,
    total_connections: u64,
}

impl StatsManager {
    pub fn new(config: StatsConfig) -> (Self, mpsc::Sender<StatEvent>) {
        let (tx, rx) = mpsc::channel(config.max_events_per_second as usize);

        let manager = Self {
            stats: Arc::new(Mutex::new(HashMap::new())),
            event_rx: rx,
            config,
            total_connections: 0,
        };

        (manager, tx)
    }

    pub async fn run(&mut self, mut shutdown_rx: tokio::sync::watch::Receiver<bool>) {
        let mut cleanup_interval = tokio::time::interval(self.config.cleanup_interval);

        loop {
            tokio::select! {
                shutdown = shutdown_rx.changed() => {
                    match shutdown {
                        Ok(_) => {
                            info!("Stats manager shutting down");
                            // Ensure all events are processed before shutting down
                            while let Ok(event) = self.event_rx.try_recv() {
                                self.handle_event(event).await;
                            }
                            break;
                        }
                        Err(e) => {
                            warn!("Shutdown channel closed: {}", e);
                            break;
                        }
                    }
                }

                Some(event) = self.event_rx.recv() => {
                    self.handle_event(event).await;
                }

                _ = cleanup_interval.tick() => {
                    self.cleanup_idle_stats().await;
                }
            }
        }

        info!("Stats manager shutdown complete");
    }

    async fn handle_event(&mut self, event: StatEvent) {
        let mut stats = self.stats.lock().await;

        match event {
            StatEvent::ClientConnected(addr) => {
                let client_stats = stats.entry(addr).or_default();
                client_stats.active_connections = client_stats.active_connections.saturating_add(1);
                client_stats.last_active = SystemTime::now();
                self.total_connections = self.total_connections.saturating_add(1);
                debug!("Client connected from {}", addr);
            }

            StatEvent::ClientDisconnected(addr) => {
                if let Some(client_stats) = stats.get_mut(&addr) {
                    client_stats.active_connections =
                        client_stats.active_connections.saturating_sub(1);
                    client_stats.last_active = SystemTime::now();
                    debug!("Client disconnected from {}", addr);
                }
            }

            StatEvent::RequestProcessed {
                addr,
                success,
                duration_ms,
            } => {
                let client_stats = stats.entry(addr).or_default();
                client_stats.total_requests = client_stats.total_requests.saturating_add(1);

                if !success {
                    client_stats.total_errors = client_stats.total_errors.saturating_add(1);
                    client_stats.last_error = Some(SystemTime::now());
                }

                // Update average response time using exponential moving average
                const ALPHA: f64 = 0.1; // Smoothing factor

                if client_stats.avg_response_time_ms == 0 {
                    client_stats.avg_response_time_ms = duration_ms;
                } else {
                    let current_avg = client_stats.avg_response_time_ms as f64;
                    client_stats.avg_response_time_ms =
                        (current_avg + ALPHA * (duration_ms as f64 - current_avg)) as u64;
                }

                client_stats.last_active = SystemTime::now();
            }

            StatEvent::QueryStats { addr, response_tx } => {
                if let Some(stats) = stats.get(&addr)
                    && response_tx.send(stats.clone()).is_err()
                {
                    warn!("Failed to send stats for {}", addr);
                }
            }

            StatEvent::QueryConnectionStats { response_tx } => {
                let conn_stats = ConnectionStats::from_client_stats(&stats);
                if response_tx.send(conn_stats).is_err() {
                    warn!("Failed to send connection stats");
                }
            }
        }
    }

    async fn cleanup_idle_stats(&self) {
        let mut stats = self.stats.lock().await;
        let now = SystemTime::now();

        stats.retain(|addr, client_stats| {
            // Check if client has been idle for too long
            let is_idle = now
                .duration_since(client_stats.last_active)
                .map(|idle_time| idle_time <= self.config.idle_timeout)
                .unwrap_or(true);

            // Check if there was an error that's old enough to clean up
            let has_recent_error = client_stats
                .last_error
                .and_then(|last_error| now.duration_since(last_error).ok())
                .map(|error_time| error_time <= self.config.error_timeout)
                .unwrap_or(false);

            let should_retain = is_idle || has_recent_error;

            if !should_retain {
                debug!(
                    "Cleaning up stats for {}: {} connections, {} requests, {} errors",
                    addr,
                    client_stats.active_connections,
                    client_stats.total_requests,
                    client_stats.total_errors
                );
            }

            should_retain
        });
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use tokio::{sync::oneshot, time::sleep};

    #[tokio::test]
    async fn test_client_lifecycle() {
        let config = StatsConfig::default();
        let (mut manager, tx) = StatsManager::new(config);
        let addr = "127.0.0.1:8080".parse().unwrap();

        let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
        let manager_handle = tokio::spawn(async move {
            manager.run(shutdown_rx).await;
        });

        // Test connection
        tx.send(StatEvent::ClientConnected(addr)).await.unwrap();

        // Test successful request
        tx.send(StatEvent::RequestProcessed {
            addr,
            success: true,
            duration_ms: Duration::from_millis(100).as_millis() as u64,
        })
        .await
        .unwrap();

        // Test failed request
        tx.send(StatEvent::RequestProcessed {
            addr,
            success: false,
            duration_ms: Duration::from_millis(150).as_millis() as u64,
        })
        .await
        .unwrap();

        sleep(Duration::from_millis(100)).await;

        // Query per-client stats
        let (response_tx, response_rx) = oneshot::channel();
        tx.send(StatEvent::QueryStats { addr, response_tx })
            .await
            .unwrap();

        let stats = response_rx.await.unwrap();
        assert_eq!(stats.active_connections, 1);
        assert_eq!(stats.total_requests, 2);
        assert_eq!(stats.total_errors, 1);

        // Query global stats
        let (response_tx, response_rx) = oneshot::channel();
        tx.send(StatEvent::QueryConnectionStats { response_tx })
            .await
            .unwrap();

        let conn_stats = response_rx.await.unwrap();
        assert_eq!(conn_stats.total_requests, 2);
        assert_eq!(conn_stats.total_errors, 1);

        // Cleanup
        shutdown_tx.send(true).unwrap();
        manager_handle.await.unwrap();
    }

    #[tokio::test]
    async fn test_cleanup_idle_stats() {
        let mut config = StatsConfig::default();
        config.idle_timeout = Duration::from_millis(100);
        let (mut manager, tx) = StatsManager::new(config);
        let addr = "127.0.0.1:8080".parse().unwrap();

        let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
        let manager_handle = tokio::spawn(async move {
            manager.run(shutdown_rx).await;
        });

        // Add client and disconnect
        tx.send(StatEvent::ClientConnected(addr)).await.unwrap();
        tx.send(StatEvent::ClientDisconnected(addr)).await.unwrap();

        // Wait for idle timeout
        sleep(Duration::from_millis(200)).await;

        // Query stats - should be cleaned up
        let (response_tx, response_rx) = oneshot::channel();
        tx.send(StatEvent::QueryConnectionStats { response_tx })
            .await
            .unwrap();

        let conn_stats = response_rx.await.unwrap();
        assert_eq!(conn_stats.active_connections, 0);

        shutdown_tx.send(true).unwrap();
        manager_handle.await.unwrap();
    }
}
