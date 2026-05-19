//! Read-only access to the auth-service `annie_auth.oauth_credentials` table from the bot.
//!
//! The auth-service (see `../auth/src/routes/authorized.rs`) is the source of
//! truth for OAuth-linked AniList accounts. The bot shares the same Postgres
//! database and reads `annie_auth.oauth_credentials` directly via raw SQL so commands like
//! `/whoami` and the guild-overlay query can recognize users that linked
//! through the OAuth flow.
//!
//! See [`crate::commands::register`] for how the link is initiated and
//! [`crate::commands::unregister`] for the cleanup-side counterpart of this
//! contract. The shared schema contract is documented in
//! `docs/oauth-contract.md`.
//!
//! `annie_auth.oauth_credentials.discord_user_id` is `TEXT` and contains the raw Discord
//! snowflake string (`user.id.get().to_string()`); `anilist_id` is `BIGINT` and
//! `anilist_username` is nullable `TEXT`.

use crate::utils::{database::DbPool, privacy::hash_user_id};
use serenity::model::prelude::UserId;
use sqlx::FromRow;
use std::fmt;
use tracing::instrument;

#[derive(Clone, PartialEq, Eq, FromRow)]
pub struct OAuthCredential {
    /// Raw Discord snowflake stored as TEXT in the auth-service schema.
    pub discord_user_id: String,
    pub anilist_id: i64,
    pub anilist_username: Option<String>,
}

impl fmt::Debug for OAuthCredential {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OAuthCredential")
            .field("discord_user_id", &"[REDACTED]")
            .field("anilist_id", &"[REDACTED]")
            .field(
                "anilist_username",
                &self.anilist_username.as_ref().map(|_| "[REDACTED]"),
            )
            .finish()
    }
}

impl OAuthCredential {
    /// Display label for the linked AniList account.
    ///
    /// Prefers the AniList username populated by the auth service, while
    /// preserving the numeric ID fallback for older rows that do not have one.
    pub fn anilist_display_name(&self) -> String {
        self.anilist_username.as_deref().map_or_else(
            || format!("AniList account ID {}", self.anilist_id),
            str::to_owned,
        )
    }

    /// Public AniList profile URL for the linked account.
    ///
    /// Uses the username URL when available and falls back to the numeric
    /// profile URL for existing credential rows without `anilist_username`.
    pub fn anilist_profile_url(&self) -> String {
        match self.anilist_username.as_deref() {
            Some(username) => format!("https://anilist.co/user/{username}/"),
            None => format!("https://anilist.co/user/{}/", self.anilist_id),
        }
    }

    /// Look up an OAuth credential row for the given Discord snowflake.
    ///
    /// Returns `Ok(None)` when no row exists for the user. Takes the
    /// snowflake as a `UserId` so callers do not have to perform a lossy
    /// `u64 as i64` cast that would silently drop snowflakes with the high
    /// bit set.
    #[instrument(
        name = "db.oauth_credential.get_by_discord_id",
        skip(pool, user_discord_id),
        fields(discord_user_id = %hash_user_id(user_discord_id.get()))
    )]
    pub async fn get_by_discord_id(
        user_discord_id: UserId,
        pool: &DbPool,
    ) -> Result<Option<OAuthCredential>, sqlx::Error> {
        sqlx::query_as::<_, OAuthCredential>(
            "SELECT discord_user_id, anilist_id, anilist_username FROM annie_auth.oauth_credentials WHERE discord_user_id = $1"
        )
        .bind(user_discord_id.get().to_string())
        .fetch_optional(pool)
        .await
    }

    /// Look up OAuth credential rows for any of the given Discord snowflakes.
    ///
    /// Useful for batch lookups across a guild. Returns the rows that exist;
    /// missing users are simply absent from the result.
    #[instrument(
        name = "db.oauth_credential.get_by_discord_ids",
        skip(pool, user_discord_ids),
        fields(user_count = user_discord_ids.len())
    )]
    pub async fn get_by_discord_ids(
        user_discord_ids: Vec<UserId>,
        pool: &DbPool,
    ) -> Result<Vec<OAuthCredential>, sqlx::Error> {
        let ids: Vec<String> = user_discord_ids
            .iter()
            .map(|id| id.get().to_string())
            .collect();

        sqlx::query_as::<_, OAuthCredential>(
            "SELECT discord_user_id, anilist_id, anilist_username FROM annie_auth.oauth_credentials \
             WHERE discord_user_id = ANY($1)",
        )
        .bind(ids)
        .fetch_all(pool)
        .await
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

    fn oauth_credential(anilist_username: Option<&str>) -> OAuthCredential {
        OAuthCredential {
            discord_user_id: "987654321".to_string(),
            anilist_id: 4567,
            anilist_username: anilist_username.map(str::to_owned),
        }
    }

    #[test]
    fn anilist_display_fields_use_username_when_available() {
        let credential = oauth_credential(Some("AniUser"));

        assert_eq!(credential.anilist_display_name(), "AniUser");
        assert_eq!(
            credential.anilist_profile_url(),
            "https://anilist.co/user/AniUser/"
        );
    }

    #[test]
    fn anilist_display_fields_fall_back_to_id_without_username() {
        let credential = oauth_credential(None);

        assert_eq!(credential.anilist_display_name(), "AniList account ID 4567");
        assert_eq!(
            credential.anilist_profile_url(),
            "https://anilist.co/user/4567/"
        );
    }

    #[test]
    fn discord_id_u64_parses_valid_snowflake() {
        let credential = OAuthCredential {
            discord_user_id: "987654321".to_string(),
            anilist_id: 1,
            anilist_username: Some("AniUser".to_string()),
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
            anilist_username: None,
        };
        assert_eq!(credential.discord_id_u64(), Some(u64::MAX));
    }

    #[test]
    fn discord_id_u64_returns_none_for_invalid_snowflake() {
        let credential = OAuthCredential {
            discord_user_id: "not-a-number".to_string(),
            anilist_id: 1,
            anilist_username: None,
        };
        assert_eq!(credential.discord_id_u64(), None);
    }

    #[test]
    fn debug_redacts_identifiers() {
        let credential = OAuthCredential {
            discord_user_id: "987654321".to_string(),
            anilist_id: 4567,
            anilist_username: Some("AniUser".to_string()),
        };

        let debug = format!("{credential:?}");

        assert!(debug.contains("[REDACTED]"));
        assert!(!debug.contains("987654321"));
        assert!(!debug.contains("4567"));
        assert!(!debug.contains("AniUser"));
    }
}
