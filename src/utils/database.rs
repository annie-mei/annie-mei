use crate::utils::statics::DATABASE_URL;

use diesel::pg::PgConnection;
use diesel::prelude::*;
use diesel::r2d2::{ConnectionManager, Pool, PooledConnection};
use diesel::sql_query;
use diesel_migrations::{EmbeddedMigrations, MigrationHarness, embed_migrations};
use serenity::{client::Context, prelude::TypeMapKey};
use std::env;
use std::time::Duration;
use tracing::{error, info, instrument};

pub type DbPool = Pool<ConnectionManager<PgConnection>>;
pub type DbConnection = PooledConnection<ConnectionManager<PgConnection>>;

pub struct DatabasePoolKey;

impl TypeMapKey for DatabasePoolKey {
    type Value = DbPool;
}

#[instrument(name = "db.create_pool", skip_all)]
pub fn create_pool() -> DbPool {
    let database_url = env::var(DATABASE_URL).expect("DATABASE_URL must be set");
    let manager = ConnectionManager::<PgConnection>::new(database_url.clone());

    Pool::builder()
        .max_size(2)
        .min_idle(Some(0))
        .test_on_check_out(true)
        .max_lifetime(Some(Duration::from_secs(20 * 60)))
        .idle_timeout(Some(Duration::from_secs(60)))
        .build(manager)
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

#[instrument(name = "db.get_connection", skip(pool))]
pub fn get_connection(pool: &DbPool) -> DbConnection {
    pool.get().unwrap_or_else(|error| {
        error!(error = %error, "Failed to get database connection from pool");
        panic!("Error retrieving pooled database connection: {error}")
    })
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

#[instrument(name = "db.ping", skip_all)]
pub fn ping(pool: &DbPool) -> Result<(), diesel::result::Error> {
    let mut conn = get_connection(pool);
    sql_query("SELECT 1").execute(&mut conn)?;
    info!("Database ping successful");
    Ok(())
}

pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!();

#[instrument(name = "db.run_migrations", skip(conn))]
pub fn run_migration(conn: &mut PgConnection) {
    info!("Running database migrations ... ");
    conn.run_pending_migrations(MIGRATIONS).unwrap();
}
