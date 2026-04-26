//! Dependency-injection traits for command handlers.
//!
//! These traits abstract external services (AniList, DB, Redis, …) so that
//! command core logic can be unit-tested with mock implementations.
//!
//! ## Example: testing the core handler directly
//!
//! ```ignore
//! use crate::commands::{anime::command::handle_anime, response::CommandResponse};
//!
//! // Not-found path — no anime, no guild data, no variant signal.
//! let response = handle_anime(None, None, None);
//! assert!(response.is_content());
//!
//! // Success path — pass a pre-built Anime, optional guild data, and the
//! // matched title variant (so the embed surfaces the user's typed variant).
//! let response = handle_anime(Some(sample_anime), Some(guild_data), Some(title_variant));
//! assert!(response.is_embed());
//! ```

use std::future::Future;

use crate::models::{
    anilist_anime::Anime, anilist_character::Character, anilist_common::TitleVariant,
    anilist_manga::Manga,
};

/// Abstraction over media-data retrieval (AniList today, pluggable tomorrow).
///
/// Implement this trait for production (real API) and test (mocked data)
/// variants. Command core-logic functions accept `&impl MediaDataSource` so
/// they never touch the network directly.
///
/// Each fetch returns the matched media along with the [`TitleVariant`] that
/// best matched the user's input, so handlers can pick the matching variant
/// for the embed title (and demote the other to the footer).
pub trait MediaDataSource: Send + Sync {
    /// Fetch anime data for the given search term (name **or** numeric ID).
    ///
    /// Returns `None` when no matching anime is found.
    fn fetch_anime(
        &self,
        search_term: &str,
    ) -> impl Future<Output = Option<(Anime, TitleVariant)>> + Send;

    /// Fetch manga data for the given search term (name **or** numeric ID).
    ///
    /// Returns `None` when no matching manga is found.
    fn fetch_manga(
        &self,
        search_term: &str,
    ) -> impl Future<Output = Option<(Manga, TitleVariant)>> + Send;
}

pub trait CharacterDataSource: Send + Sync {
    /// Fetch character data for the given search term (name **or** numeric ID).
    ///
    /// Returns `None` when no matching character is found.
    fn fetch_character(&self, search_term: &str) -> impl Future<Output = Option<Character>> + Send;
}

/// Production [`MediaDataSource`] backed by the AniList GraphQL API.
///
/// This delegates to the existing [`crate::utils::response_fetcher::fetcher`]
/// pipeline, preserving current caching (Redis) and fuzzy-match behaviour.
pub struct AniListSource;

impl MediaDataSource for AniListSource {
    async fn fetch_anime(&self, search_term: &str) -> Option<(Anime, TitleVariant)> {
        use crate::models::media_type::MediaType;
        use crate::utils::response_fetcher::fetcher;
        use serenity::all::CommandDataOptionValue;

        let arg = CommandDataOptionValue::String(search_term.to_string());
        fetcher::<Anime>(MediaType::Anime, arg).await
    }

    async fn fetch_manga(&self, search_term: &str) -> Option<(Manga, TitleVariant)> {
        use crate::models::media_type::MediaType;
        use crate::utils::response_fetcher::fetcher;
        use serenity::all::CommandDataOptionValue;

        let arg = CommandDataOptionValue::String(search_term.to_string());
        fetcher::<Manga>(MediaType::Manga, arg).await
    }
}

impl CharacterDataSource for AniListSource {
    async fn fetch_character(&self, search_term: &str) -> Option<Character> {
        use crate::utils::response_fetcher::character_fetcher;
        use serenity::all::CommandDataOptionValue;

        let arg = CommandDataOptionValue::String(search_term.to_string());
        character_fetcher(arg).await
    }
}
