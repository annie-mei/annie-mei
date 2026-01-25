use crate::{
    commands::songs::fetcher::fetcher as SongFetcher,
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
use tracing::info;

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

pub async fn run(ctx: &Context, interaction: &mut CommandInteraction) {
    let _ = interaction.defer(&ctx.http).await;

    let user = &interaction.user;
    let arg = interaction.data.options[0].value.clone();
    let arg_str = format!("{:?}", arg);

    configure_sentry_scope("Songs", user.id.get(), Some(json!(arg_str)));

    info!("Got command 'songs' with args: {arg:#?}");

    let response = task::spawn_blocking(move || SongFetcher(arg))
        .await
        .unwrap();

    let _songs_response = match response {
        None => {
            let builder = EditInteractionResponse::new().content(NOT_FOUND_ANIME);
            interaction.edit_response(&ctx.http, builder).await
        }
        Some(song_response) => {
            let builder = EditInteractionResponse::new()
                .embed(build_message_from_song_response(song_response));
            interaction.edit_response(&ctx.http, builder).await
        }
    };
}

fn build_message_from_song_response(mal_response: MalResponse) -> CreateEmbed {
    CreateEmbed::new()
        .title(mal_response.transform_title())
        .field("Openings", mal_response.transform_openings(), false)
        .field("Endings", mal_response.transform_endings(), false)
        .thumbnail(mal_response.transform_thumbnail())
        .field("\u{200b}", mal_response.transform_mal_link(), false)
}
