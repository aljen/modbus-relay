use std::{collections::HashMap, net::SocketAddr, sync::Arc, time::Duration};

use tokio::sync::{Mutex, Semaphore, mpsc, oneshot};
use tracing::error;

use crate::{ConnectionError, RelayError, config::ConnectionConfig};

use super::{ConnectionGuard, ConnectionStats, StatEvent};

/// TCP connection management
#[derive(Debug)]
pub struct Manager {
    /// Connection limit per IP
    per_ip_semaphores: Arc<Mutex<HashMap<SocketAddr, Arc<Semaphore>>>>,
    /// Global connection limit
    global_semaphore: Arc<Semaphore>,
    /// Active connections counter per IP
    active_connections: Arc<Mutex<HashMap<SocketAddr, usize>>>,
    /// Configuration
    config: ConnectionConfig,
    /// Stats event sender
    stats_tx: mpsc::Sender<StatEvent>,
}

impl Manager {
    pub fn new(config: ConnectionConfig, stats_tx: mpsc::Sender<StatEvent>) -> Self {
        Self {
            per_ip_semaphores: Arc::new(Mutex::new(HashMap::new())),
            global_semaphore: Arc::new(Semaphore::new(config.max_connections as usize)),
            active_connections: Arc::new(Mutex::new(HashMap::new())),
            config,
            stats_tx,
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

        // Increment active connections counter
        {
            let mut active_conns = self.active_connections.lock().await;
            let conn_count = active_conns.entry(addr).or_default();
            *conn_count = conn_count.saturating_add(1);
        }

        // Notify stats manager about new connection
        if let Err(e) = self.stats_tx.send(StatEvent::ClientConnected(addr)).await {
            error!("Failed to send connection event to stats manager: {}", e);
        }

        Ok(ConnectionGuard {
            manager: Arc::clone(self),
            addr,
            _global_permit: global_permit,
            _per_ip_permit: per_ip_permit,
        })
    }

    pub async fn get_connection_count(&self, addr: &SocketAddr) -> usize {
        self.active_connections
            .lock()
            .await
            .get(addr)
            .copied()
            .unwrap_or(0)
    }

    pub async fn get_total_connections(&self) -> usize {
        self.active_connections.lock().await.values().sum()
    }

    /// Updates statistics for a given request
    pub async fn record_request(&self, addr: SocketAddr, success: bool, duration: Duration) {
        if let Err(e) = self
            .stats_tx
            .send(StatEvent::RequestProcessed {
                addr,
                success,
                duration_ms: duration.as_millis() as u64,
            })
            .await
        {
            error!("Failed to send request stats: {}", e);
        }
    }

    /// Gets complete connection statistics
    pub async fn get_stats(&self) -> Result<ConnectionStats, RelayError> {
        let (tx, rx) = oneshot::channel();

        self.stats_tx
            .send(StatEvent::QueryConnectionStats { response_tx: tx })
            .await
            .map_err(|_| {
                RelayError::Connection(ConnectionError::invalid_state(
                    "Failed to query connection stats",
                ))
            })?;

        rx.await.map_err(|_| {
            RelayError::Connection(ConnectionError::invalid_state(
                "Failed to receive connection stats",
            ))
        })
    }

    /// Cleans up idle connections
    pub(crate) async fn cleanup_idle_connections(&self) -> Result<(), RelayError> {
        // Cleanup is now handled by StatsManager, we just need to sync our active connections
        let (tx, rx) = oneshot::channel();

        self.stats_tx
            .send(StatEvent::QueryConnectionStats { response_tx: tx })
            .await
            .map_err(|_| {
                RelayError::Connection(ConnectionError::invalid_state(
                    "Failed to query stats for cleanup",
                ))
            })?;

        let stats = rx.await.map_err(|_| {
            RelayError::Connection(ConnectionError::invalid_state(
                "Failed to receive stats for cleanup",
            ))
        })?;

        let mut active_conns = self.active_connections.lock().await;
        active_conns.retain(|addr, count| {
            if let Some(ip_stats) = stats.per_ip_stats.get(addr) {
                ip_stats.active_connections > 0
            } else {
                // If no stats exist, connection is considered inactive
                *count == 0
            }
        });

        Ok(())
    }

    pub(crate) fn decrease_connection_count(&self, addr: SocketAddr) {
        let mut active_conns = self
            .active_connections
            .try_lock()
            .expect("Failed to lock active_connections in guard drop");

        if let Some(count) = active_conns.get_mut(&addr) {
            *count = count.saturating_sub(1);
            if *count == 0 {
                active_conns.remove(&addr);
            }
        }
    }

    pub fn stats_tx(&self) -> mpsc::Sender<StatEvent> {
        self.stats_tx.clone()
    }
}
