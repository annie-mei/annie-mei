use std::env;
use std::net::SocketAddr;

use axum::{Json, Router, http::StatusCode, routing::get};
use serde_json::{Value, json};
use tokio::net::TcpListener;
use tracing::{error, info, instrument};

use crate::utils::statics::{DEFAULT_SERVER_PORT, SERVER_PORT};

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[instrument(name = "http.healthz", skip_all)]
async fn healthz() -> (StatusCode, Json<Value>) {
    let result = tokio::task::spawn_blocking(crate::utils::redis::ping).await;

    let (status, redis_ok) = match result {
        Ok(Ok(())) => (StatusCode::OK, true),
        Ok(Err(e)) => {
            error!(error = %e, "Redis health check failed");
            (StatusCode::SERVICE_UNAVAILABLE, false)
        }
        Err(e) => {
            error!(error = %e, "Health check task panicked");
            (StatusCode::INTERNAL_SERVER_ERROR, false)
        }
    };

    let body = json!({
        "status": if status == StatusCode::OK { "healthy" } else { "unhealthy" },
        "version": VERSION,
        "services": {
            "redis": if redis_ok { "up" } else { "down" },
        }
    });

    (status, Json(body))
}

#[instrument(name = "http.server", skip_all)]
pub async fn run(shutdown: tokio::sync::watch::Receiver<()>) -> std::io::Result<()> {
    let port = env::var(SERVER_PORT)
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(DEFAULT_SERVER_PORT);

    let app = Router::new().route("/healthz", get(healthz));

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = TcpListener::bind(addr).await?;

    info!(%addr, "HTTP server listening");

    axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            let mut shutdown = shutdown;
            let _ = shutdown.changed().await;
            info!("HTTP server shutting down");
        })
        .await
}
