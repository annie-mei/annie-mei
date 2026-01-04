use crate::utils::statics::DATABASE_URL;

use diesel::pg::PgConnection;
use diesel::prelude::*;
use diesel_migrations::{EmbeddedMigrations, MigrationHarness, embed_migrations};
use std::env;
use tracing::info;

pub fn establish_connection() -> PgConnection {
    let database_url = env::var(DATABASE_URL).expect("DATABASE_URL must be set");
    PgConnection::establish(&database_url)
        .unwrap_or_else(|_| panic!("Error connecting to {database_url}"))
}

pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!();
pub fn run_migration(conn: &mut PgConnection) {
    info!("Running database migrations ... ");
    conn.run_pending_migrations(MIGRATIONS).unwrap();
}
