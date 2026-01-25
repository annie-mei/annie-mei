use crate::{
    models::{anilist_anime::Anime, media_type::MediaType as Type, transformers::Transformers},
    utils::{
        guild::{get_current_guild_members, get_guild_data_for_media},
        privacy::configure_sentry_scope,
        response_fetcher::fetcher,
        statics::NOT_FOUND_ANIME,
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
use tracing::info;

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

pub async fn run(ctx: &Context, interaction: &mut CommandInteraction) {
    let _ = interaction.defer(&ctx.http).await;

    let user = &interaction.user;
    let arg = interaction.data.options[0].value.clone();
    let arg_str = format!("{:?}", arg);

    configure_sentry_scope("Anime", user.id.get(), Some(json!(arg_str)));

    info!("Got command 'anime' with args: {arg:#?}");

    let response: Option<Anime> = task::spawn_blocking(move || fetcher(Type::Anime, arg))
        .await
        .unwrap();

    let _anime_response = match response {
        None => {
            let builder = EditInteractionResponse::new().content(NOT_FOUND_ANIME);
            interaction.edit_response(&ctx.http, builder).await
        }
        Some(anime_response) => {
            // TODO: Refactor this to fetcher.rs

            let guild_members = get_current_guild_members(ctx, interaction);
            let also_anime = anime_response.clone();

            let guild_members_data = if guild_members.is_empty() {
                info!("No users found in guild");
                None
            } else {
                let guild_members_data = task::spawn_blocking(move || {
                    get_guild_data_for_media(also_anime, guild_members)
                })
                .await
                .unwrap()
                .await;
                info!("Guild members data: {:#?}", guild_members_data);
                if guild_members_data.is_empty() {
                    None
                } else {
                    Some(guild_members_data)
                }
            };

            let anime_response_embed = anime_response.transform_response_embed(guild_members_data);

            let builder = EditInteractionResponse::new().embed(anime_response_embed);
            interaction.edit_response(&ctx.http, builder).await
        }
    };
}
