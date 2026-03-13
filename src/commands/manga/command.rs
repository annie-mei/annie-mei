use std::collections::HashMap;

use crate::{
    commands::{
        input_validation::{MAX_SEARCH_INPUT_LEN, validate_search_option},
        response::CommandResponse,
        traits::{AniListSource, MediaDataSource},
    },
    models::{anilist_manga::Manga, transformers::Transformers, user_media_list::MediaListData},
    utils::{
        channel::is_nsfw_channel,
        guild::{get_current_guild_members, get_guild_data_for_media},
        privacy::configure_sentry_scope,
        statics::{NOT_FOUND_MANGA, NSFW_NOT_ALLOWED},
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
    CreateCommand::new("manga")
        .description("Fetches the details for a manga")
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

/// Decide the `/manga` response from already-fetched data.
///
/// This is the testable entry-point — it never touches `Context` or
/// `CommandInteraction`.  The adapter is responsible for fetching the manga
/// (via [`MediaDataSource`]) and guild-member data before calling this.
///
/// `guild_members_data` is optional guild-member score data that the adapter
/// fetches separately (it requires Discord cache access).
pub fn handle_manga(
    manga: Option<Manga>,
    guild_members_data: Option<HashMap<i64, MediaListData>>,
) -> CommandResponse {
    match manga {
        None => CommandResponse::Content(NOT_FOUND_MANGA.to_string()),
        Some(manga_response) => {
            let embed = manga_response.transform_response_embed(guild_members_data);
            CommandResponse::Embed(Box::new(embed))
        }
    }
}

// ── Serenity adapter (thin wrapper) ─────────────────────────────────────

#[instrument(name = "command.manga.run", skip(ctx, interaction))]
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
        "Manga",
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
        search_len, "Got command 'manga' with validated search input"
    );

    // Fetch manga data on a blocking thread (AniList uses blocking reqwest).
    let manga_result: Option<Manga> =
        match task::spawn_blocking(move || AniListSource.fetch_manga(&search_term)).await {
            Ok(result) => result,
            Err(e) => {
                error!(
                    error = %e,
                    search_kind,
                    search_len,
                    "spawn_blocking panicked while fetching manga"
                );
                let builder = EditInteractionResponse::new().content(
                    "I couldn't fetch manga details right now. Please try again in a few minutes.",
                );
                let _ = interaction.edit_response(&ctx.http, builder).await;
                return;
            }
        };

    // Block adult content in non-NSFW channels.
    if let Some(ref manga) = manga_result
        && manga.is_adult()
        && !is_nsfw_channel(ctx, interaction.channel_id).await
    {
        let builder = EditInteractionResponse::new().content(NSFW_NOT_ALLOWED);
        let _ = interaction.edit_response(&ctx.http, builder).await;
        return;
    }

    // Gather guild-member data when the manga was found.
    let guild_members_data = match &manga_result {
        None => None,
        Some(manga_response) => {
            let guild_members = get_current_guild_members(ctx, interaction);
            if guild_members.is_empty() {
                info!(search_kind, search_len, "No users found in guild");
                None
            } else {
                let also_manga = manga_response.clone();
                let data = get_guild_data_for_media(ctx, also_manga, guild_members).await;
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
    let response = handle_manga(manga_result, guild_members_data);

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

    /// Helper: build a minimal `Manga` from JSON for testing.
    fn sample_manga() -> Manga {
        serde_json::from_value(serde_json::json!({
            "type": "MANGA",
            "id": 30013,
            "idMal": 13,
            "isAdult": false,
            "title": {
                "romaji": "One Piece",
                "english": "One Piece",
                "native": "ワンピース"
            },
            "synonyms": [],
            "startDate": { "year": 1997, "month": 7, "day": 22 },
            "endDate": null,
            "format": "MANGA",
            "status": "RELEASING",
            "chapters": null,
            "volumes": null,
            "genres": ["Action", "Adventure", "Comedy", "Drama", "Fantasy"],
            "source": "ORIGINAL",
            "coverImage": {
                "extraLarge": "https://example.com/cover.jpg",
                "large": null,
                "medium": null,
                "color": "#e4a015"
            },
            "averageScore": 92,
            "staff": {
                "edges": [{ "role": "Story & Art" }],
                "nodes": [{ "name": { "full": "Eiichiro Oda" } }]
            },
            "siteUrl": "https://anilist.co/manga/30013",
            "externalLinks": [],
            "description": "<p>Gol D. Roger was known as the Pirate King.</p>",
            "tags": [{ "name": "Shounen" }]
        }))
        .expect("sample manga JSON should deserialize")
    }

    #[test]
    fn manga_not_found_returns_content_with_message() {
        let response = handle_manga(None, None);

        assert!(response.is_content(), "expected Content variant");
        assert_eq!(response.unwrap_content(), NOT_FOUND_MANGA);
    }

    #[test]
    fn manga_success_returns_embed() {
        let response = handle_manga(Some(sample_manga()), None);

        assert!(
            response.is_embed(),
            "expected Embed variant for a successful lookup"
        );
        let _embed = response.unwrap_embed();
    }

    #[test]
    fn manga_success_with_no_guild_data_still_returns_embed() {
        let response = handle_manga(Some(sample_manga()), None);

        assert!(response.is_embed());
    }
}
