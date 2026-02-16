use crate::{
    models::{anilist_manga::Manga, media_type::MediaType as Type, transformers::Transformers},
    utils::{
        guild::{get_current_guild_members, get_guild_data_for_media},
        privacy::configure_sentry_scope,
        response_fetcher::fetcher,
        statics::NOT_FOUND_MANGA,
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

#[instrument(name = "command.manga.run", skip(ctx, interaction))]
pub async fn run(ctx: &Context, interaction: &mut CommandInteraction) {
    let _ = interaction.defer(&ctx.http).await;

    let user = &interaction.user;
    let arg = interaction.data.options[0].value.clone();
    let arg_str = format!("{:?}", arg);

    configure_sentry_scope("Manga", user.id.get(), Some(json!(arg_str)));

    info!("Got command 'manga' with args: {arg:#?}");

    let response: Option<Manga> = task::spawn_blocking(move || fetcher(Type::Manga, arg))
        .await
        .unwrap();

    let _manga_response = match response {
        None => {
            let builder = EditInteractionResponse::new().content(NOT_FOUND_MANGA);
            interaction.edit_response(&ctx.http, builder).await
        }
        Some(manga_response) => {
            // TODO: Refactor this to fetcher.rs

            let guild_members = get_current_guild_members(ctx, interaction);
            let also_manga = manga_response.clone();

            let guild_members_data = if guild_members.is_empty() {
                info!("No users found in guild");
                None
            } else {
                let guild_members_data = task::spawn_blocking(move || {
                    get_guild_data_for_media(also_manga, guild_members)
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

            let manga_response_embed = manga_response.transform_response_embed(guild_members_data);

            let builder = EditInteractionResponse::new().embed(manga_response_embed);
            interaction.edit_response(&ctx.http, builder).await
        }
    };
}
