use crate::utils::statics::DATABASE_URL;

use serenity::{client::Context, prelude::TypeMapKey};
use sqlx::{Pool, Postgres, postgres::PgPoolOptions};
use std::env;
use std::time::Duration;
use tracing::{error, info, instrument};

pub type DbPool = Pool<Postgres>;

pub struct DatabasePoolKey;

impl TypeMapKey for DatabasePoolKey {
    type Value = DbPool;
}

#[instrument(name = "db.create_pool", skip_all)]
pub async fn create_pool() -> DbPool {
    let database_url = env::var(DATABASE_URL).expect("DATABASE_URL must be set");

    PgPoolOptions::new()
        .max_connections(10)
        .min_connections(0)
        .max_lifetime(Duration::from_secs(20 * 60))
        .idle_timeout(Duration::from_secs(60))
        .test_before_acquire(true)
        .connect(&database_url)
        .await
        .unwrap_or_else(|error| {
            let redacted_url = redact_database_url(&database_url);
            error!(
                error = %error,
                database_url = %redacted_url,
                "Failed to create database connection pool"
            );
            panic!("Error creating pool for {redacted_url}: {error}")
        })
}

#[instrument(name = "db.run_migrations", skip(pool))]
pub async fn run_migrations(pool: &DbPool) -> Result<(), sqlx::migrate::MigrateError> {
    ensure_annie_mei_schema(pool).await?;

    let mut connection = pool.acquire().await?;
    sqlx::query("SET search_path TO annie_mei, annie_auth, public")
        .execute(&mut *connection)
        .await?;

    sqlx::migrate!("./migrations").run(&mut *connection).await?;

    sqlx::query("RESET search_path")
        .execute(&mut *connection)
        .await?;

    info!("Database migrations completed");
    Ok(())
}

#[instrument(name = "db.ensure_annie_mei_schema", skip(pool))]
async fn ensure_annie_mei_schema(pool: &DbPool) -> Result<(), sqlx::Error> {
    let schema_exists: Option<String> =
        sqlx::query_scalar("SELECT to_regnamespace('annie_mei')::text")
            .fetch_one(pool)
            .await?;

    if schema_exists.is_some() {
        return Ok(());
    }

    match sqlx::query("CREATE SCHEMA annie_mei").execute(pool).await {
        Ok(_) => Ok(()),
        Err(sqlx::Error::Database(error)) if error.code().as_deref() == Some("42P06") => Ok(()),
        Err(error) => Err(error),
    }
}

#[instrument(name = "db.pool_from_context", skip(ctx))]
pub async fn get_pool_from_context(ctx: &Context) -> Option<DbPool> {
    let data = ctx.data.read().await;
    data.get::<DatabasePoolKey>().cloned()
}

fn redact_database_url(database_url: &str) -> String {
    let scheme_end = database_url.find("://").map(|pos| pos + 3);
    let at_sign = database_url.rfind('@');

    match (scheme_end, at_sign) {
        (Some(start), Some(end)) if start < end => {
            let mut redacted = String::with_capacity(database_url.len());
            redacted.push_str(&database_url[..start]);
            redacted.push_str("**redacted**@");
            redacted.push_str(&database_url[end + 1..]);
            redacted
        }
        _ => database_url.to_string(),
    }
}
