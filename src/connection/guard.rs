use std::{net::SocketAddr, sync::Arc};

use super::ConnectionManager;

/// RAII guard for the connection
#[derive(Debug)]
pub struct ConnectionGuard {
    pub manager: Arc<ConnectionManager>,
    pub addr: SocketAddr,
    pub _global_permit: tokio::sync::OwnedSemaphorePermit,
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
