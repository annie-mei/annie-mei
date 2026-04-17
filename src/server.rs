use std::env;
use std::net::SocketAddr;

use axum::{Json, Router, extract::State, http::StatusCode, routing::get};
use serde_json::{Value, json};
use tokio::net::TcpListener;
use tracing::{error, info, instrument};

use crate::utils::statics::{DEFAULT_SERVER_PORT, SERVER_PORT};

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[instrument(name = "http.healthz.redis_blocking", skip_all)]
fn run_redis_health_check() -> redis::RedisResult<()> {
    crate::utils::redis::ping()
}

#[instrument(name = "http.healthz.database_blocking", skip_all)]
fn run_database_health_check(
    database_pool: &crate::utils::database::DbPool,
) -> Result<(), diesel::result::Error> {
    crate::utils::database::ping(database_pool)
}

#[instrument(name = "http.healthz", skip_all)]
async fn healthz(
    State(database_pool): State<crate::utils::database::DbPool>,
) -> (StatusCode, Json<Value>) {
    let health_check_pool = database_pool.clone();
    let (redis_result, db_result) = tokio::join!(
        tokio::task::spawn_blocking(run_redis_health_check),
        tokio::task::spawn_blocking(move || run_database_health_check(&health_check_pool)),
    );

    let redis_ok = match &redis_result {
        Ok(Ok(())) => true,
        Ok(Err(e)) => {
            error!(error = %e, "Redis health check failed");
            false
        }
        Err(e) => {
            error!(error = %e, "Redis health check task panicked");
            false
        }
    };

    let db_ok = match &db_result {
        Ok(Ok(())) => true,
        Ok(Err(e)) => {
            error!(error = %e, "Database health check failed");
            false
        }
        Err(e) => {
            error!(error = %e, "Database health check task panicked");
            false
        }
    };

    let all_healthy = redis_ok && db_ok;
    let status = if all_healthy {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };

    let body = json!({
        "status": if all_healthy { "healthy" } else { "unhealthy" },
        "version": VERSION,
        "services": {
            "redis": if redis_ok { "up" } else { "down" },
            "database": if db_ok { "up" } else { "down" },
        }
    });

    (status, Json(body))
}

#[instrument(name = "http.server", skip_all)]
pub async fn run(
    shutdown: tokio::sync::watch::Receiver<()>,
    database_pool: crate::utils::database::DbPool,
) -> std::io::Result<()> {
    let port = env::var(SERVER_PORT)
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(DEFAULT_SERVER_PORT);

    let app = Router::new()
        .route("/healthz", get(healthz))
        .with_state(database_pool);

    let addr = SocketAddr::from(([127, 0, 0, 1], port));
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
