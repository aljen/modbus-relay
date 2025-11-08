use super::{ConnectionStats, stats::ClientStats};
use std::net::SocketAddr;
use tokio::sync::oneshot;

#[derive(Debug)]
pub enum StatEvent {
    /// Client connected from address
    ClientConnected(SocketAddr),
    /// Client disconnected from address
    ClientDisconnected(SocketAddr),
    /// Request processed with success/failure and duration
    RequestProcessed {
        addr: SocketAddr,
        success: bool,
        duration_ms: u64,
    },
    /// Query stats for specific address
    QueryStats {
        addr: SocketAddr,
        response_tx: oneshot::Sender<ClientStats>,
    },
    /// Query global connection stats
    QueryConnectionStats {
        response_tx: oneshot::Sender<ConnectionStats>,
    },
}
