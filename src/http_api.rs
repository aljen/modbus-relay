use std::sync::Arc;

use axum::{extract::State, http::StatusCode, response::IntoResponse, routing::get, Json, Router};
use serde::Serialize;
use tokio::sync::broadcast;
use tracing::info;

use crate::ConnectionManager;

#[derive(Debug, Serialize)]
struct HealthResponse {
    status: &'static str,
    tcp_connections: u32,
    rtu_status: &'static str,
}

#[derive(Debug, Serialize)]
struct StatsResponse {
    total_requests: u64,
    active_connections: u32,
    error_count: u32,
    avg_response_time_ms: u64,
    requests_per_second: f64,
}

type ApiState = Arc<ConnectionManager>;

async fn health_handler(State(state): State<ApiState>) -> impl IntoResponse {
    let response = HealthResponse {
        status: "ok",
        tcp_connections: state.connection_count().await,
        rtu_status: "ok", // TODO: Implement RTU status check
    };

    (StatusCode::OK, Json(response))
}

async fn stats_handler(State(state): State<ApiState>) -> impl IntoResponse {
    let response = StatsResponse {
        total_requests: state.total_requests(),
        active_connections: state.connection_count().await,
        error_count: state.error_count(),
        avg_response_time_ms: state.avg_response_time().await.as_millis() as u64,
        requests_per_second: state.requests_per_second(),
    };

    (StatusCode::OK, Json(response))
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
