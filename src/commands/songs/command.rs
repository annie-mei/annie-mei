use crate::{
    commands::{
        input_validation::validate_search_term,
        songs::fetcher::{SongFetchResult, fetcher as SongFetcher},
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

struct SongEmbedData {
    title: String,
    openings: String,
    endings: String,
    thumbnail: String,
    mal_link: String,
}

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
    let arg = interaction.data.options[0].value.clone();
    let arg_str = format!("{:?}", arg);

    configure_sentry_scope("Songs", user.id.get(), Some(json!(arg_str)));

    info!("Got command 'songs' with args: {arg:#?}");

    if let serenity::all::CommandDataOptionValue::String(ref search_term) = arg
        && let Err(err) = validate_search_term(search_term)
    {
        let builder = EditInteractionResponse::new().content(format!(
            "Invalid search input: {err}. Please check your input and try again."
        ));
        let _ = interaction.edit_response(&ctx.http, builder).await;
        return;
    }

    let response = SongFetcher(arg).await;

    let _songs_response = match response {
        SongFetchResult::Found(song_response) => {
            let embed_data =
                match task::spawn_blocking(move || build_message_from_song_response(song_response))
                    .await
                {
                    Ok(data) => data,
                    Err(err) => {
                        error!(error = %err, "spawn_blocking panicked while building song embed");
                        let builder = EditInteractionResponse::new().content(
                        "An internal error occurred while fetching songs. Please try again later.",
                    );
                        let _ = interaction.edit_response(&ctx.http, builder).await;
                        return;
                    }
                };

            let builder = EditInteractionResponse::new().embed(
                CreateEmbed::new()
                    .title(embed_data.title)
                    .field("Openings", embed_data.openings, false)
                    .field("Endings", embed_data.endings, false)
                    .thumbnail(embed_data.thumbnail)
                    .field("\u{200b}", embed_data.mal_link, false),
            );
            interaction.edit_response(&ctx.http, builder).await
        }
        SongFetchResult::AnimeNotFound => {
            let builder = EditInteractionResponse::new().content(NOT_FOUND_ANIME);
            interaction.edit_response(&ctx.http, builder).await
        }
        SongFetchResult::AnimeNotFoundOnMal => {
            let builder = EditInteractionResponse::new()
                .content("Anime not found on MAL. Song data is only available for anime listed on MyAnimeList.");
            interaction.edit_response(&ctx.http, builder).await
        }
        SongFetchResult::FetchError => {
            let builder = EditInteractionResponse::new()
                .content("An error occurred while fetching song data. Please try again later.");
            interaction.edit_response(&ctx.http, builder).await
        }
    };
}

#[instrument(name = "command.songs.build_message", skip(mal_response))]
fn build_message_from_song_response(mal_response: MalResponse) -> SongEmbedData {
    SongEmbedData {
        title: mal_response.transform_title(),
        openings: mal_response.transform_openings(),
        endings: mal_response.transform_endings(),
        thumbnail: mal_response.transform_thumbnail(),
        mal_link: mal_response.transform_mal_link(),
    }
}
