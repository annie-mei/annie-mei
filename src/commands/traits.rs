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
//! // Not-found path — no anime, no guild data.
//! let response = handle_anime(None, None);
//! assert!(response.is_content());
//!
//! // Success path — pass a pre-built Anime and optional guild data.
//! let response = handle_anime(Some(sample_anime), Some(guild_data));
//! assert!(response.is_embed());
//! ```

use crate::models::anilist_anime::Anime;

/// Abstraction over media-data retrieval (AniList today, pluggable tomorrow).
///
/// Implement this trait for production (real API) and test (mocked data)
/// variants. Command core-logic functions accept `&impl MediaDataSource` so
/// they never touch the network directly.
pub trait MediaDataSource: Send + Sync {
    /// Fetch anime data for the given search term (name **or** numeric ID).
    ///
    /// Returns `None` when no matching anime is found.
    fn fetch_anime(&self, search_term: &str) -> Option<Anime>;
}

/// Production [`MediaDataSource`] backed by the AniList GraphQL API.
///
/// This delegates to the existing [`crate::utils::response_fetcher::fetcher`]
/// pipeline, preserving current caching (Redis) and fuzzy-match behaviour.
pub struct AniListSource;

impl MediaDataSource for AniListSource {
    fn fetch_anime(&self, search_term: &str) -> Option<Anime> {
        use crate::models::media_type::MediaType;
        use crate::utils::response_fetcher::fetcher;
        use serenity::all::CommandDataOptionValue;

        let arg = CommandDataOptionValue::String(search_term.to_string());
        fetcher::<Anime>(MediaType::Anime, arg)
    }
}
