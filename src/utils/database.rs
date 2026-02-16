use crate::utils::statics::DATABASE_URL;

use diesel::pg::PgConnection;
use diesel::prelude::*;
use diesel_migrations::{EmbeddedMigrations, MigrationHarness, embed_migrations};
use std::env;
use tracing::{error, info, instrument};

#[instrument(name = "db.establish_connection", skip_all)]
pub fn establish_connection() -> PgConnection {
    let database_url = env::var(DATABASE_URL).expect("DATABASE_URL must be set");
    PgConnection::establish(&database_url).unwrap_or_else(|error| {
        let redacted_url = redact_database_url(&database_url);
        error!(
            error = %error,
            database_url = %redacted_url,
            "Failed to connect to database"
        );
        panic!("Error connecting to {redacted_url}: {error}")
    })
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

pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!();

#[instrument(name = "db.run_migrations", skip(conn))]
pub fn run_migration(conn: &mut PgConnection) {
    info!("Running database migrations ... ");
    conn.run_pending_migrations(MIGRATIONS).unwrap();
}
