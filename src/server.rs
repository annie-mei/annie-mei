use std::env;
use std::net::SocketAddr;

use axum::{
    Json, Router,
    extract::State,
    http::{HeaderMap, StatusCode},
    routing::get,
};
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

fn header_value<'a>(headers: &'a HeaderMap, name: &str) -> &'a str {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("unknown")
}

#[instrument(name = "http.healthz.metadata", skip_all)]
fn log_health_request(endpoint: &str, headers: &HeaderMap) {
    info!(
        endpoint,
        user_agent = header_value(headers, "user-agent"),
        cf_ray = header_value(headers, "cf-ray"),
        "Health endpoint requested"
    );
}

fn build_health_response(redis_ok: bool, db_ok: Option<bool>) -> (StatusCode, Json<Value>) {
    let all_healthy = redis_ok && db_ok.unwrap_or(true);
    let status = if all_healthy {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };

    let database_status = match db_ok {
        Some(true) => "up",
        Some(false) => "down",
        None => "not_checked",
    };

    let body = json!({
        "status": if all_healthy { "healthy" } else { "unhealthy" },
        "version": VERSION,
        "services": {
            "redis": if redis_ok { "up" } else { "down" },
            "database": database_status,
        }
    });

    (status, Json(body))
}

fn evaluate_redis_health(
    redis_result: &Result<redis::RedisResult<()>, tokio::task::JoinError>,
) -> bool {
    match redis_result {
        Ok(Ok(())) => true,
        Ok(Err(e)) => {
            error!(error = %e, "Redis health check failed");
            false
        }
        Err(e) => {
            error!(error = %e, "Redis health check task panicked");
            false
        }
    }
}

fn evaluate_database_health(
    db_result: &Result<Result<(), diesel::result::Error>, tokio::task::JoinError>,
) -> bool {
    match db_result {
        Ok(Ok(())) => true,
        Ok(Err(e)) => {
            error!(error = %e, "Database health check failed");
            false
        }
        Err(e) => {
            error!(error = %e, "Database health check task panicked");
            false
        }
    }
}

#[instrument(name = "http.healthz", skip_all)]
async fn healthz(headers: HeaderMap) -> (StatusCode, Json<Value>) {
    log_health_request("/healthz", &headers);

    let redis_result = tokio::task::spawn_blocking(run_redis_health_check).await;
    let redis_ok = evaluate_redis_health(&redis_result);

    build_health_response(redis_ok, None)
}

#[instrument(name = "http.readyz", skip_all)]
async fn readyz(
    State(database_pool): State<crate::utils::database::DbPool>,
    headers: HeaderMap,
) -> (StatusCode, Json<Value>) {
    log_health_request("/readyz", &headers);

    let health_check_pool = database_pool.clone();
    let (redis_result, db_result) = tokio::join!(
        tokio::task::spawn_blocking(run_redis_health_check),
        tokio::task::spawn_blocking(move || run_database_health_check(&health_check_pool)),
    );

    let redis_ok = evaluate_redis_health(&redis_result);
    let db_ok = evaluate_database_health(&db_result);

    build_health_response(redis_ok, Some(db_ok))
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
        .route("/readyz", get(readyz))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn healthz_response_reports_healthy_when_redis_is_up_and_database_is_not_checked() {
        let (status, Json(body)) = build_health_response(true, None);

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["status"], "healthy");
        assert_eq!(body["services"]["redis"], "up");
        assert_eq!(body["services"]["database"], "not_checked");
        assert_eq!(body["version"], VERSION);
    }

    #[test]
    fn healthz_response_reports_unhealthy_when_redis_is_down() {
        let (status, Json(body)) = build_health_response(false, None);

        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(body["status"], "unhealthy");
        assert_eq!(body["services"]["redis"], "down");
        assert_eq!(body["services"]["database"], "not_checked");
    }

    #[test]
    fn readyz_response_reports_healthy_when_all_services_are_up() {
        let (status, Json(body)) = build_health_response(true, Some(true));

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["status"], "healthy");
        assert_eq!(body["services"]["redis"], "up");
        assert_eq!(body["services"]["database"], "up");
    }

    #[test]
    fn readyz_response_reports_unhealthy_when_database_is_down() {
        let (status, Json(body)) = build_health_response(true, Some(false));

        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(body["status"], "unhealthy");
        assert_eq!(body["services"]["redis"], "up");
        assert_eq!(body["services"]["database"], "down");
    }
}
