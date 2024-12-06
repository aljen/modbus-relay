use std::{net::SocketAddr, sync::Arc};
use tokio::sync::OwnedSemaphorePermit;
use tracing::debug;

use super::ConnectionManager;

/// RAII guard for the connection
#[derive(Debug)]
pub struct ConnectionGuard {
    pub manager: Arc<ConnectionManager>,
    pub addr: SocketAddr,
    pub _global_permit: OwnedSemaphorePermit,
    pub _per_ip_permit: Option<OwnedSemaphorePermit>,
}

impl Drop for ConnectionGuard {
    fn drop(&mut self) {
        let manager = self.manager.clone();
        let addr = self.addr;

        debug!("Closing connection from {}", addr);

        tokio::spawn(async move {
            manager.decrease_connection_count(addr).await;
        });
    }
}
