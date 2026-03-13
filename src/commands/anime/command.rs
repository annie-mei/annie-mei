use std::collections::HashMap;

use crate::{
    commands::{
        input_validation::{MAX_SEARCH_INPUT_LEN, validate_search_option},
        response::CommandResponse,
        traits::{AniListSource, MediaDataSource},
    },
    models::{anilist_anime::Anime, transformers::Transformers, user_media_list::MediaListData},
    utils::{
        channel::is_nsfw_channel,
        guild::{get_current_guild_members, get_guild_data_for_media},
        privacy::configure_sentry_scope,
        statics::{NOT_FOUND_ANIME, NSFW_NOT_ALLOWED},
    },
};

use serde_json::json;
use serenity::{
    all::{CommandInteraction, CreateCommandOption, EditInteractionResponse},
    builder::CreateCommand,
    client::Context,
    model::application::CommandOptionType,
};

use tokio::task;
use tracing::{error, info, instrument};

pub fn register() -> CreateCommand {
    CreateCommand::new("anime")
        .description("Fetches the details for an anime")
        .add_option(
            CreateCommandOption::new(
                CommandOptionType::String,
                "search",
                "Anilist ID or Search term",
            )
            .required(true),
        )
}

// ── Core logic (transport-agnostic) ─────────────────────────────────────

/// Decide the `/anime` response from already-fetched data.
///
/// This is the testable entry-point — it never touches `Context` or
/// `CommandInteraction`.  The adapter is responsible for fetching the anime
/// (via [`MediaDataSource`]) and guild-member data before calling this.
///
/// `guild_members_data` is optional guild-member score data that the adapter
/// fetches separately (it requires Discord cache access).
pub fn handle_anime(
    anime: Option<Anime>,
    guild_members_data: Option<HashMap<i64, MediaListData>>,
) -> CommandResponse {
    match anime {
        None => CommandResponse::Content(NOT_FOUND_ANIME.to_string()),
        Some(anime_response) => {
            let embed = anime_response.transform_response_embed(guild_members_data);
            CommandResponse::Embed(Box::new(embed))
        }
    }
}

// ── Serenity adapter (thin wrapper) ─────────────────────────────────────

#[instrument(name = "command.anime.run", skip(ctx, interaction))]
pub async fn run(ctx: &Context, interaction: &mut CommandInteraction) {
    let _ = interaction.defer(&ctx.http).await;

    let user = &interaction.user;

    let validated_search =
        match validate_search_option(&interaction.data.options, "search", MAX_SEARCH_INPUT_LEN) {
            Ok(validated_search) => validated_search,
            Err(error) => {
                let builder = EditInteractionResponse::new().content(error.user_message());
                let _ = interaction.edit_response(&ctx.http, builder).await;
                return;
            }
        };
    let search_kind = validated_search.kind.as_str();
    let search_term = validated_search.value;
    let search_len = search_term.len();

    configure_sentry_scope(
        "Anime",
        user.id.get(),
        Some(json!({
            "search": {
                "kind": search_kind,
                "len": search_len,
            }
        })),
    );

    info!(
        search_kind,
        search_len, "Got command 'anime' with validated search input"
    );

    // Fetch anime data on a blocking thread (AniList uses blocking reqwest).
    let anime_result: Option<Anime> =
        match task::spawn_blocking(move || AniListSource.fetch_anime(&search_term)).await {
            Ok(result) => result,
            Err(e) => {
                error!(
                    error = %e,
                    search_kind,
                    search_len,
                    "spawn_blocking panicked while fetching anime"
                );
                let builder = EditInteractionResponse::new().content(
                    "I couldn't fetch anime details right now. Please try again in a few minutes.",
                );
                let _ = interaction.edit_response(&ctx.http, builder).await;
                return;
            }
        };

    // Block adult content in non-NSFW channels.
    if let Some(ref anime) = anime_result
        && anime.is_adult()
        && !is_nsfw_channel(ctx, interaction.channel_id).await
    {
        let builder = EditInteractionResponse::new().content(NSFW_NOT_ALLOWED);
        let _ = interaction.edit_response(&ctx.http, builder).await;
        return;
    }

    // Gather guild-member data when the anime was found.
    let guild_members_data = match &anime_result {
        None => None,
        Some(anime_response) => {
            let guild_members = get_current_guild_members(ctx, interaction);
            if guild_members.is_empty() {
                info!(search_kind, search_len, "No users found in guild");
                None
            } else {
                let also_anime = anime_response.clone();
                let data = get_guild_data_for_media(ctx, also_anime, guild_members).await;
                info!(
                    search_kind,
                    search_len,
                    guild_members_data_len = data.len(),
                    "Guild members data fetched"
                );
                if data.is_empty() { None } else { Some(data) }
            }
        }
    };

    // Delegate to the transport-agnostic core logic.
    let response = handle_anime(anime_result, guild_members_data);

    // Map the CommandResponse to the appropriate Discord API call.
    let _result = match response {
        CommandResponse::Content(text) => {
            let builder = EditInteractionResponse::new().content(text);
            interaction.edit_response(&ctx.http, builder).await
        }
        CommandResponse::Embed(embed) => {
            let builder = EditInteractionResponse::new().embed(*embed);
            interaction.edit_response(&ctx.http, builder).await
        }
        CommandResponse::Message(text) => {
            let builder = EditInteractionResponse::new().content(text);
            interaction.edit_response(&ctx.http, builder).await
        }
    };
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: build a minimal `Anime` from JSON for testing.
    fn sample_anime() -> Anime {
        serde_json::from_value(serde_json::json!({
            "type": "ANIME",
            "id": 21,
            "idMal": 21,
            "isAdult": false,
            "title": {
                "romaji": "One Piece",
                "english": "One Piece",
                "native": "ワンピース"
            },
            "synonyms": [],
            "season": "FALL",
            "seasonYear": 1999,
            "format": "TV",
            "status": "RELEASING",
            "episodes": null,
            "duration": 24,
            "genres": ["Action", "Adventure", "Comedy", "Drama", "Fantasy"],
            "source": "MANGA",
            "coverImage": {
                "extraLarge": "https://example.com/cover.jpg",
                "large": null,
                "medium": null,
                "color": "#e4a015"
            },
            "averageScore": 88,
            "studios": {
                "edges": [{ "isMain": true }],
                "nodes": [{ "name": "Toei Animation" }]
            },
            "siteUrl": "https://anilist.co/anime/21",
            "externalLinks": [
                { "url": "https://www.crunchyroll.com/one-piece", "type": "STREAMING" }
            ],
            "trailer": { "id": "abc123", "site": "youtube" },
            "description": "<p>Gold Roger was known as the Pirate King.</p>",
            "tags": [{ "name": "Shounen" }]
        }))
        .expect("sample anime JSON should deserialize")
    }

    #[test]
    fn anime_not_found_returns_content_with_message() {
        let response = handle_anime(None, None);

        assert!(response.is_content(), "expected Content variant");
        assert_eq!(response.unwrap_content(), NOT_FOUND_ANIME);
    }

    #[test]
    fn anime_success_returns_embed() {
        let response = handle_anime(Some(sample_anime()), None);

        assert!(
            response.is_embed(),
            "expected Embed variant for a successful lookup"
        );
        // The embed was built — we trust `transform_response_embed` for
        // field-level correctness (it's covered by its own tests).
        let _embed = response.unwrap_embed();
    }

    #[test]
    fn anime_success_with_no_guild_data_still_returns_embed() {
        let response = handle_anime(Some(sample_anime()), None);

        assert!(response.is_embed());
    }
}
