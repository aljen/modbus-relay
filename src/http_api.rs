use std::{collections::HashMap, net::SocketAddr, sync::Arc, time::SystemTime};

use axum::{extract::State, http::StatusCode, response::IntoResponse, routing::get, Json, Router};
use serde::Serialize;
use tokio::sync::{broadcast, oneshot};
use tracing::info;

use crate::{connection::StatEvent, ConnectionManager};

#[derive(Debug, Serialize)]
struct HealthResponse {
    status: &'static str,
    tcp_connections: u32,
    rtu_status: &'static str,
}

#[derive(Debug, Serialize)]
struct IpStatsResponse {
    active_connections: usize,
    total_requests: u64,
    total_errors: u64,
    avg_response_time_ms: u64,
    last_active: SystemTime,
    last_error: Option<SystemTime>,
}

#[derive(Debug, Serialize)]
struct StatsResponse {
    // Basic stats
    total_connections: u64,
    active_connections: u32,
    total_requests: u64,
    total_errors: u64,
    requests_per_second: f64,
    avg_response_time_ms: u64,

    // Stats per IP
    per_ip_stats: HashMap<SocketAddr, IpStatsResponse>,
}

type ApiState = Arc<ConnectionManager>;

async fn health_handler(State(state): State<ApiState>) -> impl IntoResponse {
    let (tx, rx) = oneshot::channel();

    if (state
        .stats_tx()
        .send(StatEvent::QueryConnectionStats { response_tx: tx })
        .await)
        .is_err()
    {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(HealthResponse {
                status: "error",
                tcp_connections: 0,
                rtu_status: "unknown",
            }),
        );
    }

    match rx.await {
        Ok(stats) => {
            (
                StatusCode::OK,
                Json(HealthResponse {
                    status: "ok",
                    tcp_connections: stats.active_connections as u32,
                    rtu_status: "ok", // TODO(aljen): Implement RTU status check
                }),
            )
        }
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(HealthResponse {
                status: "error",
                tcp_connections: 0,
                rtu_status: "unknown",
            }),
        ),
    }
}

async fn stats_handler(State(state): State<ApiState>) -> impl IntoResponse {
    let (tx, rx) = oneshot::channel();

    if (state
        .stats_tx()
        .send(StatEvent::QueryConnectionStats { response_tx: tx })
        .await)
        .is_err()
    {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(StatsResponse {
                total_connections: 0,
                active_connections: 0,
                total_requests: 0,
                total_errors: 0,
                requests_per_second: 0.0,
                avg_response_time_ms: 0,
                per_ip_stats: HashMap::new(),
            }),
        );
    }

    match rx.await {
        Ok(stats) => {
            let per_ip_stats = stats
                .per_ip_stats
                .into_iter()
                .map(|(addr, ip_stats)| {
                    (
                        addr,
                        IpStatsResponse {
                            active_connections: ip_stats.active_connections,
                            total_requests: ip_stats.total_requests,
                            total_errors: ip_stats.total_errors,
                            avg_response_time_ms: ip_stats.avg_response_time_ms,
                            last_active: ip_stats.last_active,
                            last_error: ip_stats.last_error,
                        },
                    )
                })
                .collect();

            (
                StatusCode::OK,
                Json(StatsResponse {
                    total_connections: stats.total_connections,
                    active_connections: stats.active_connections as u32,
                    total_requests: stats.total_requests,
                    total_errors: stats.total_errors,
                    requests_per_second: stats.requests_per_second,
                    avg_response_time_ms: stats.avg_response_time_ms,
                    per_ip_stats,
                }),
            )
        }
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(StatsResponse {
                total_connections: 0,
                active_connections: 0,
                total_requests: 0,
                total_errors: 0,
                requests_per_second: 0.0,
                avg_response_time_ms: 0,
                per_ip_stats: HashMap::new(),
            }),
        ),
    }
}

pub async fn start_http_server(
    address: String,
    port: u16,
    manager: Arc<ConnectionManager>,
    mut shutdown_rx: broadcast::Receiver<()>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let app = Router::new()
        .route("/health", get(health_handler))
        .route("/stats", get(stats_handler))
        .with_state(manager);

    let addr = format!("{}:{}", address, port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;

    info!("HTTP server listening on {}", addr);

    axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            let _ = shutdown_rx.recv().await;
            info!("HTTP server shutting down");
        })
        .await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::{ConnectionConfig, StatsManager};

    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use tokio::sync::Mutex;
    use tower::ServiceExt;

    #[tokio::test]
    async fn test_health_endpoint() {
        // Create a test stats manager
        let config = ConnectionConfig::default();
        let stats_config = crate::StatsConfig::default();
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

        // Build test app
        let app = Router::new()
            .route("/health", get(health_handler))
            .with_state(manager);

        // Create test request
        let req = Request::builder()
            .uri("/health")
            .body(Body::empty())
            .unwrap();

        // Get response
        let response = app.oneshot(req).await.unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        shutdown_tx.send(()).unwrap();
        stats_handle.await.unwrap();
    }

    #[tokio::test]
    async fn test_stats_endpoint() {
        let config = ConnectionConfig::default();
        let stats_config = crate::StatsConfig::default();
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

        let app = Router::new()
            .route("/stats", get(stats_handler))
            .with_state(manager);

        let req = Request::builder()
            .uri("/stats")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(req).await.unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        shutdown_tx.send(()).unwrap();
        stats_handle.await.unwrap();
    }
}
