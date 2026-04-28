use std::collections::HashMap;

use crate::{
    commands::{
        response::CommandResponse,
        traits::{AniListSource, MediaDataSource},
    },
    models::{
        anilist_common::TitleVariant, anilist_manga::Manga, transformers::Transformers,
        user_media_list::MediaListData,
    },
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

use tracing::{info, instrument};

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
    title_variant: Option<TitleVariant>,
) -> CommandResponse {
    match manga {
        None => CommandResponse::Content(NOT_FOUND_MANGA.to_string()),
        Some(manga_response) => {
            let embed = manga_response.transform_response_embed(guild_members_data, title_variant);
            CommandResponse::Embed(Box::new(embed))
        }
    }
}

// ── Serenity adapter (thin wrapper) ─────────────────────────────────────

#[instrument(name = "command.manga.run", skip(ctx, interaction))]
pub async fn run(ctx: &Context, interaction: &mut CommandInteraction) {
    let _ = interaction.defer(&ctx.http).await;

    let user = &interaction.user;

    // Validate the required "search" option up-front.
    let Some(serenity::all::CommandDataOptionValue::String(search_term)) =
        interaction.data.options.first().map(|opt| &opt.value)
    else {
        let builder = EditInteractionResponse::new()
            .content("Missing or invalid `search` option — please provide a manga name or ID.");
        let _ = interaction.edit_response(&ctx.http, builder).await;
        return;
    };
    let search_term = search_term.clone();

    configure_sentry_scope("Manga", user.id.get(), Some(json!(search_term.clone())));

    info!("Got command 'manga' with search_term: {search_term}");

    let fetch_result: Option<(Manga, TitleVariant)> = AniListSource.fetch_manga(&search_term).await;
    let (manga_result, title_variant): (Option<Manga>, Option<TitleVariant>) = match fetch_result {
        Some((manga, variant)) => (Some(manga), Some(variant)),
        None => (None, None),
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
                info!("No users found in guild");
                None
            } else {
                let data = get_guild_data_for_media(ctx, manga_response, guild_members).await;
                info!("Guild members data: {} entries", data.len());
                if data.is_empty() { None } else { Some(data) }
            }
        }
    };

    // Delegate to the transport-agnostic core logic.
    let response = handle_manga(manga_result, guild_members_data, title_variant);

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
            "description": "<p>Gol D. Roger was known as the Pirate King.</p>",
            "tags": [{ "name": "Shounen" }]
        }))
        .expect("sample manga JSON should deserialize")
    }

    #[test]
    fn manga_not_found_returns_content_with_message() {
        let response = handle_manga(None, None, None);

        assert!(response.is_content(), "expected Content variant");
        assert_eq!(response.unwrap_content(), NOT_FOUND_MANGA);
    }

    #[test]
    fn manga_success_returns_embed() {
        let response = handle_manga(Some(sample_manga()), None, None);

        assert!(
            response.is_embed(),
            "expected Embed variant for a successful lookup"
        );
        let _embed = response.unwrap_embed();
    }

    #[test]
    fn manga_success_with_no_guild_data_still_returns_embed() {
        let response = handle_manga(Some(sample_manga()), None, None);

        assert!(response.is_embed());
    }
}
