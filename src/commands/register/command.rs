use crate::{
    models::db::user::User,
    utils::{database, privacy::configure_sentry_scope},
};

use serde_json::json;
use serenity::{
    all::{CommandInteraction, CreateCommandOption, EditInteractionResponse, ResolvedValue},
    builder::CreateCommand,
    client::Context,
    model::application::CommandOptionType,
};
use tokio::task;
use tracing::info;

pub fn register() -> CreateCommand {
    CreateCommand::new("register")
        .description("Command to register your user's Anilist account")
        .add_option(
            CreateCommandOption::new(CommandOptionType::String, "anilist", "Anilist username")
                .required(true),
        )
}

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

    let response_message = register_new_user(anilist_username.to_owned(), user).await;

    let builder = EditInteractionResponse::new().content(response_message);
    let _register = interaction.edit_response(&ctx.http, builder).await;
}

async fn register_new_user(anilist_username: String, user: &serenity::model::user::User) -> String {
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

    let connection = &mut database::establish_connection();

    {
        let anilist_id = anilist_id.unwrap();
        User::create_or_update_user(
            user.id.get() as i64,
            anilist_id,
            anilist_username.to_owned(),
            connection,
        );

        info!(
            "Created user with details: id: {}, anilist_id: {}, anilist_username: {}",
            user.id, anilist_id, anilist_username
        );
        format!(
            "Hello {}, I have linked the Anilist account {} to your user.",
            user.name, anilist_username
        )
    }
}
