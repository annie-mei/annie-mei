use std::env;
use std::net::SocketAddr;

use axum::{Router, http::StatusCode, routing::get};
use tokio::net::TcpListener;
use tracing::{error, info, instrument};

use crate::utils::statics::{DEFAULT_SERVER_PORT, SERVER_PORT};

#[instrument(name = "http.healthz", skip_all)]
async fn healthz() -> StatusCode {
    let result = tokio::task::spawn_blocking(crate::utils::redis::ping).await;

    match result {
        Ok(Ok(())) => StatusCode::OK,
        Ok(Err(e)) => {
            error!(error = %e, "Redis health check failed");
            StatusCode::SERVICE_UNAVAILABLE
        }
        Err(e) => {
            error!(error = %e, "Health check task panicked");
            StatusCode::INTERNAL_SERVER_ERROR
        }
    }
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
