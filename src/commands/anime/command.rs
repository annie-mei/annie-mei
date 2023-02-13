use crate::{
    models::{anilist_anime::Anime, media_type::MediaType as Type, transformers::Transformers},
    utils::{
        guild::{get_current_guild_members, get_guild_data_for_media},
        response_fetcher::fetcher,
        statics::NOT_FOUND_ANIME,
    },
};

use serenity::{
    builder::CreateApplicationCommand,
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
        .name("anime")
        .description("Fetches the details for an anime")
        .create_option(|option| {
            option
                .name("id")
                .description("Anilist ID")
                .kind(CommandOptionType::Integer)
                .min_int_value(1)
        })
        .create_option(|option| {
            option
                .name("name")
                .description("Search term")
                .kind(CommandOptionType::String)
        })
}

pub async fn run(ctx: &Context, interaction: &mut ApplicationCommandInteraction) {
    let user = &interaction.user;
    let arg = interaction.data.options[0].resolved.to_owned().unwrap();

    info!(
        "Got command 'anime' by user '{}' with args: {arg:#?}",
        user.name,
    );

    let response: Option<Anime> = task::spawn_blocking(move || fetcher(Type::Anime, arg))
        .await
        .unwrap();

    let _anime_response = match response {
        None => {
            interaction
                .create_interaction_response(&ctx.http, |response| {
                    { response.kind(InteractionResponseType::ChannelMessageWithSource) }
                        .interaction_response_data(|m| m.content(NOT_FOUND_ANIME))
                })
                .await
        }
        Some(anime_response) => {
            // TODO: Refactor this to fetcher.rs

            let guild_members = get_current_guild_members(ctx, interaction);
            let also_anime = anime_response.clone();

            let scores = if guild_members.is_empty() {
                info!("No users found in guild");
                None
            } else {
                let scores = task::spawn_blocking(move || {
                    get_guild_data_for_media(also_anime, guild_members)
                })
                .await
                .unwrap()
                .await;
                info!("Guild scores: {:#?}", scores);
                Some(scores)
            };

            let anime_response_embed = anime_response.transform_response_embed(scores);

            interaction
                .create_interaction_response(&ctx.http, |response| {
                    { response.kind(InteractionResponseType::ChannelMessageWithSource) }
                        .interaction_response_data(|m| m.set_embed(anime_response_embed))
                })
                .await
        }
    };
}
