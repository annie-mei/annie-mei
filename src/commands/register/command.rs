use crate::{
    models::db::user::User,
    utils::{
        database,
        privacy::{configure_sentry_scope, hash_user_id},
    },
};

use serde_json::json;
use serenity::{
    all::{CommandInteraction, CreateCommandOption, EditInteractionResponse, ResolvedValue},
    builder::CreateCommand,
    client::Context,
    model::application::CommandOptionType,
};
use tokio::task;
use tracing::{error, info, instrument};

pub fn register() -> CreateCommand {
    CreateCommand::new("register")
        .description("Command to register your user's Anilist account")
        .add_option(
            CreateCommandOption::new(CommandOptionType::String, "anilist", "Anilist username")
                .required(true),
        )
}

#[instrument(name = "command.register.run", skip(ctx, interaction))]
pub async fn run(ctx: &Context, interaction: &mut CommandInteraction) {
    let _ = interaction.defer(&ctx.http).await;

    let user = &interaction.user;
    let options = interaction.data.options();
    let arg = &options[0].value;
    let arg_str = format!("{:?}", arg);

    configure_sentry_scope("Register", user.id.get(), Some(json!(arg_str)));

    info!("Got command 'register' with args: {arg:#?}");

    let anilist_username = match arg {
        ResolvedValue::String(name) => name.to_string(),
        _ => panic!("Invalid argument type"),
    };

    let Some(database_pool) = database::get_pool_from_context(ctx).await else {
        let builder = EditInteractionResponse::new()
            .content("Database is not initialized. Please try again later.");
        let _register = interaction.edit_response(&ctx.http, builder).await;
        return;
    };

    let response_message =
        register_new_user(anilist_username.to_owned(), user, database_pool).await;

    let builder = EditInteractionResponse::new().content(response_message);
    let _register = interaction.edit_response(&ctx.http, builder).await;
}

#[instrument(name = "command.register.register_new_user", skip(user, database_pool), fields(discord_user_id = %hash_user_id(user.id.get()), username_len = anilist_username.len()))]
async fn register_new_user(
    anilist_username: String,
    user: &serenity::model::user::User,
    database_pool: database::DbPool,
) -> String {
    let username = anilist_username.to_string();
    let anilist_id =
        task::spawn_blocking(move || User::get_anilist_id_from_username(username.as_ref()))
            .await
            .unwrap();

    if anilist_id.is_none() {
        return format!(
            "Hello {}, I could not find the Anilist account {}.",
            user.name, anilist_username
        );
    };

    let discord_id = user.id.get() as i64;
    let user_name = user.name.clone();
    let anilist_id = anilist_id.unwrap();
    let anilist_username_for_db = anilist_username.clone();

    let db_write_result = task::spawn_blocking(move || {
        let mut connection = database::get_connection(&database_pool);
        User::create_or_update_user(
            discord_id,
            anilist_id,
            anilist_username_for_db,
            &mut connection,
        );
    })
    .await;

    if let Err(err) = db_write_result {
        error!(
            error = %err,
            discord_user_id = %hash_user_id(discord_id as u64),
            "Failed to save user registration"
        );
        return format!(
            "Hello {}, I hit an internal error while linking your Anilist account. Please try again later.",
            user_name
        );
    }

    info!(
        discord_user_id = %hash_user_id(discord_id as u64),
        anilist_id,
        anilist_username = %anilist_username,
        "Created user with details"
    );

    format!(
        "Hello {}, I have linked the Anilist account {} to your user.",
        user_name, anilist_username
    )
}
