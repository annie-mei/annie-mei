use crate::{
    commands::{
        input_validation::{MAX_SEARCH_INPUT_LEN, validate_search_option},
        songs::fetcher::{SongFetchError, fetcher as SongFetcher},
    },
    models::mal_response::MalResponse,
    utils::{privacy::configure_sentry_scope, statics::NOT_FOUND_ANIME},
};

use serde_json::json;
use serenity::{
    all::{CommandInteraction, CreateCommandOption, CreateEmbed, EditInteractionResponse},
    builder::CreateCommand,
    client::Context,
    model::application::CommandOptionType,
};

use tokio::task;
use tracing::{error, info, instrument};

pub fn register() -> CreateCommand {
    CreateCommand::new("songs")
        .description("Fetches the songs of an anime")
        .add_option(
            CreateCommandOption::new(
                CommandOptionType::String,
                "search",
                "Anilist ID or Search term",
            )
            .required(true),
        )
}

#[instrument(name = "command.songs.run", skip(ctx, interaction))]
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
        "Songs",
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
        search_len, "Got command 'songs' with validated search input"
    );

    let response = match task::spawn_blocking(move || SongFetcher(&search_term)).await {
        Ok(response) => response,
        Err(e) => {
            error!(
                error = %e,
                search_kind,
                search_len,
                "spawn_blocking panicked while fetching songs"
            );
            let builder = EditInteractionResponse::new()
                .content("I couldn't fetch songs right now. Please try again in a few minutes.");
            let _ = interaction.edit_response(&ctx.http, builder).await;
            return;
        }
    };

    let _songs_response = match response {
        Err(SongFetchError::AnimeNotFound) => {
            let builder = EditInteractionResponse::new().content(NOT_FOUND_ANIME);
            interaction.edit_response(&ctx.http, builder).await
        }
        Err(SongFetchError::MissingMyAnimeListId) => {
            let builder = EditInteractionResponse::new().content(
                "I found that anime, but it doesn't have a MyAnimeList ID, so I couldn't fetch theme songs.",
            );
            interaction.edit_response(&ctx.http, builder).await
        }
        Err(SongFetchError::UpstreamUnavailable | SongFetchError::MalformedUpstreamResponse) => {
            let builder = EditInteractionResponse::new().content(
                "I couldn't fetch songs from MyAnimeList right now. Please try again in a few minutes.",
            );
            interaction.edit_response(&ctx.http, builder).await
        }
        Ok(song_response) => {
            let builder = EditInteractionResponse::new()
                .embed(build_message_from_song_response(song_response));
            interaction.edit_response(&ctx.http, builder).await
        }
    };
}

#[instrument(name = "command.songs.build_message", skip(mal_response))]
fn build_message_from_song_response(mal_response: MalResponse) -> CreateEmbed {
    CreateEmbed::new()
        .title(mal_response.transform_title())
        .field("Openings", mal_response.transform_openings(), false)
        .field("Endings", mal_response.transform_endings(), false)
        .thumbnail(mal_response.transform_thumbnail())
        .field("\u{200b}", mal_response.transform_mal_link(), false)
}
