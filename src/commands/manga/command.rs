use crate::{
    models::{anilist_manga::Manga, media_type::MediaType as Type, transformers::Transformers},
    utils::{
        guild::{get_current_guild_members, get_guild_data_for_media},
        response_fetcher::fetcher,
        statics::NOT_FOUND_MANGA,
    },
};

use serde_json::json;
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
        .name("manga")
        .description("Fetches the details for a manga")
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
    let json_arg = json!(arg);

    sentry::configure_scope(|scope| {
        let mut context = std::collections::BTreeMap::new();
        context.insert("Command".to_string(), "Manga".into());
        context.insert("Arg".to_string(), json_arg);
        scope.set_context("Manga", sentry::protocol::Context::Other(context));
        scope.set_user(Some(sentry::User {
            username: Some(user.name.to_string()),
            ..Default::default()
        }));
    });

    info!(
        "Got command 'manga' by user '{}' with args: {arg:#?}",
        user.name
    );

    let response: Option<Manga> = task::spawn_blocking(move || fetcher(Type::Manga, arg))
        .await
        .unwrap();

    let _manga_response = match response {
        None => {
            interaction
                .create_interaction_response(&ctx.http, |response| {
                    { response.kind(InteractionResponseType::ChannelMessageWithSource) }
                        .interaction_response_data(|m| m.content(NOT_FOUND_MANGA))
                })
                .await
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
                Some(guild_members_data)
            };

            let manga_response_embed = manga_response.transform_response_embed(guild_members_data);

            interaction
                .create_interaction_response(&ctx.http, |response| {
                    { response.kind(InteractionResponseType::ChannelMessageWithSource) }
                        .interaction_response_data(|m| m.set_embed(manga_response_embed))
                })
                .await
        }
    };
}
