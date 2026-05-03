//! Read-only access to the auth-service `oauth_credentials` table from the bot.
//!
//! The auth-service (see `../auth/src/routes/authorized.rs`) is the source of
//! truth for OAuth-linked AniList accounts. The bot shares the same Postgres
//! database and reads `oauth_credentials` directly via raw SQL so commands like
//! `/whoami` and the guild-overlay query can recognize users that linked
//! through the OAuth flow.
//!
//! See [`crate::commands::register`] for how the link is initiated and
//! [`crate::commands::unregister`] for the cleanup-side counterpart of this
//! contract. The shared schema contract is documented in
//! `docs/oauth-contract.md`.
//!
//! `oauth_credentials.discord_user_id` is `TEXT` and contains the raw Discord
//! snowflake string (`user.id.get().to_string()`); `anilist_id` is `BIGINT`.

use crate::utils::privacy::hash_user_id;
use diesel::prelude::*;
use diesel::sql_types::{BigInt, Text};
use serenity::model::prelude::UserId;
use tracing::instrument;

const SELECT_OAUTH_CREDENTIAL_BY_DISCORD_ID_SQL: &str =
    "SELECT discord_user_id, anilist_id FROM oauth_credentials WHERE discord_user_id = $1";

const SELECT_OAUTH_CREDENTIALS_BY_DISCORD_IDS_SQL: &str = "SELECT discord_user_id, anilist_id FROM oauth_credentials \
     WHERE discord_user_id = ANY($1)";

#[derive(Debug, Clone, PartialEq, Eq, QueryableByName)]
pub struct OAuthCredential {
    /// Raw Discord snowflake stored as TEXT in the auth-service schema.
    #[diesel(sql_type = Text)]
    pub discord_user_id: String,
    #[diesel(sql_type = BigInt)]
    pub anilist_id: i64,
}

impl OAuthCredential {
    /// Look up an OAuth credential row for the given Discord snowflake.
    ///
    /// Returns `Ok(None)` when no row exists for the user. Takes the
    /// snowflake as a `UserId` so callers do not have to perform a lossy
    /// `u64 as i64` cast that would silently drop snowflakes with the high
    /// bit set.
    #[instrument(
        name = "db.oauth_credential.get_by_discord_id",
        skip(conn, user_discord_id),
        fields(discord_user_id = %hash_user_id(user_discord_id.get()))
    )]
    pub fn get_by_discord_id(
        user_discord_id: UserId,
        conn: &mut PgConnection,
    ) -> Result<Option<OAuthCredential>, diesel::result::Error> {
        diesel::sql_query(SELECT_OAUTH_CREDENTIAL_BY_DISCORD_ID_SQL)
            .bind::<Text, _>(user_discord_id.get().to_string())
            .get_result::<OAuthCredential>(conn)
            .optional()
    }

    /// Look up OAuth credential rows for any of the given Discord snowflakes.
    ///
    /// Useful for batch lookups across a guild. Returns the rows that exist;
    /// missing users are simply absent from the result.
    #[instrument(
        name = "db.oauth_credential.get_by_discord_ids",
        skip(conn, user_discord_ids),
        fields(user_count = user_discord_ids.len())
    )]
    pub fn get_by_discord_ids(
        user_discord_ids: Vec<UserId>,
        conn: &mut PgConnection,
    ) -> Result<Vec<OAuthCredential>, diesel::result::Error> {
        let ids: Vec<String> = user_discord_ids
            .iter()
            .map(|id| id.get().to_string())
            .collect();

        diesel::sql_query(SELECT_OAUTH_CREDENTIALS_BY_DISCORD_IDS_SQL)
            .bind::<diesel::sql_types::Array<Text>, _>(ids)
            .get_results::<OAuthCredential>(conn)
    }

    /// Parse the stored snowflake back to a `u64` for downstream Discord APIs.
    ///
    /// Discord snowflakes are unsigned 64-bit integers; parsing as `u64`
    /// avoids silently dropping rows whose snowflake has the high bit set.
    pub fn discord_id_u64(&self) -> Option<u64> {
        self.discord_user_id.parse::<u64>().ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discord_id_u64_parses_valid_snowflake() {
        let credential = OAuthCredential {
            discord_user_id: "987654321".to_string(),
            anilist_id: 1,
        };
        assert_eq!(credential.discord_id_u64(), Some(987654321));
    }

    #[test]
    fn discord_id_u64_parses_high_bit_snowflake() {
        // Snowflakes near u64::MAX would overflow an i64 parse; u64 must
        // round-trip them so guild-overlay lookups do not silently drop the
        // user.
        let credential = OAuthCredential {
            discord_user_id: u64::MAX.to_string(),
            anilist_id: 1,
        };
        assert_eq!(credential.discord_id_u64(), Some(u64::MAX));
    }

    #[test]
    fn discord_id_u64_returns_none_for_invalid_snowflake() {
        let credential = OAuthCredential {
            discord_user_id: "not-a-number".to_string(),
            anilist_id: 1,
        };
        assert_eq!(credential.discord_id_u64(), None);
    }
}
