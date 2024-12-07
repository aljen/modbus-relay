use std::{net::SocketAddr, sync::Arc};
use tokio::sync::OwnedSemaphorePermit;
use tracing::{trace, warn};

use crate::connection::StatEvent;

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
        trace!("Dropping connection guard for {}", self.addr);

        if let Err(e) = self
            .manager
            .stats_tx()
            .try_send(StatEvent::ClientDisconnected(self.addr))
        {
            warn!("Failed to send disconnect event: {}", e);
        }

        self.manager.decrease_connection_count(self.addr);

        trace!("Connection guard dropped for {}", self.addr);
    }
}
