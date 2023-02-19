use crate::{
    commands::songs::fetcher::fetcher as SongFetcher, models::mal_response::MalResponse,
    utils::statics::NOT_FOUND_ANIME,
};

use serde_json::json;
use serenity::{
    builder::{CreateApplicationCommand, CreateEmbed},
    client::Context,
    model::{
        application::interaction::{
            application_command::ApplicationCommandInteraction, InteractionResponseType,
        },
        prelude::command::CommandOptionType,
    },
};

use tokio::task;
use tracing::info;

pub fn register(command: &mut CreateApplicationCommand) -> &mut CreateApplicationCommand {
    command
        .name("songs")
        .description("Fetches the songs of an anime")
        .create_option(|option| {
            option
                .name("search")
                .description("Anilist ID or Search term")
                .kind(CommandOptionType::String)
                .required(true)
        })
}

pub async fn run(ctx: &Context, interaction: &mut ApplicationCommandInteraction) {
    let user = &interaction.user;
    let arg = interaction.data.options[0].resolved.to_owned().unwrap();
    let json_arg = json!(arg);

    sentry::configure_scope(|scope| {
        let mut context = std::collections::BTreeMap::new();
        context.insert("Command".to_string(), "Songs".into());
        context.insert("Arg".to_string(), json_arg);
        scope.set_context("Songs", sentry::protocol::Context::Other(context));
        scope.set_user(Some(sentry::User {
            username: Some(user.name.to_string()),
            ..Default::default()
        }));
    });

    info!(
        "Got command 'songs' by user '{}' with args: {arg:#?}",
        user.name
    );

    let response = task::spawn_blocking(move || SongFetcher(arg))
        .await
        .unwrap();

    let _songs_response = match response {
        None => {
            interaction
                .create_interaction_response(&ctx.http, |response| {
                    { response.kind(InteractionResponseType::ChannelMessageWithSource) }
                        .interaction_response_data(|m| m.content(NOT_FOUND_ANIME))
                })
                .await
        }
        Some(song_response) => {
            interaction
                .create_interaction_response(&ctx.http, |response| {
                    { response.kind(InteractionResponseType::ChannelMessageWithSource) }
                        .interaction_response_data(|m| {
                            m.embed(|e| build_message_from_song_response(song_response, e))
                        })
                })
                .await
        }
    };
}

fn build_message_from_song_response(
    mal_response: MalResponse,
    embed: &mut CreateEmbed,
) -> &mut CreateEmbed {
    embed
        .title(mal_response.transform_title())
        .field("Openings", mal_response.transform_openings(), false)
        .field("Endings", mal_response.transform_endings(), false)
        .thumbnail(mal_response.transform_thumbnail())
        .field("\u{200b}", mal_response.transform_mal_link(), false)
}
