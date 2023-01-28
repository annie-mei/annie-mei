use crate::{models::db::user::User, utils::database};

use serenity::{
    builder::CreateApplicationCommand,
    client::Context,
    model::{
        application::interaction::{
            application_command::ApplicationCommandInteraction, InteractionResponseType,
        },
        prelude::{
            command::CommandOptionType,
            interaction::application_command::CommandDataOptionValue::String as StringArg,
        },
    },
};
use tokio::task;
use tracing::info;

pub fn register(command: &mut CreateApplicationCommand) -> &mut CreateApplicationCommand {
    command
        .name("register")
        .description("Command to register your user's Anilist account")
        .create_option(|option| {
            option
                .name("anilist")
                .description("Anilist username")
                .kind(CommandOptionType::String)
        })
}

pub async fn run(ctx: &Context, interaction: &mut ApplicationCommandInteraction) {
    let user = &interaction.user;
    let arg = interaction.data.options[0].resolved.to_owned().unwrap();

    info!(
        "Got command 'register' by user '{}' with args: {arg:#?}",
        user.name
    );

    let anilist_username = match arg {
        StringArg(name) => name,
        _ => panic!("Invalid argument type"),
    };

    let response_message = register_new_user(anilist_username.to_owned(), user).await;

    let _register = interaction
        .create_interaction_response(&ctx.http, |response| {
            { response.kind(InteractionResponseType::ChannelMessageWithSource) }
                .interaction_response_data(|m| m.content(response_message))
        })
        .await;
}

async fn register_new_user(anilist_username: String, user: &serenity::model::user::User) -> String {
    let username = anilist_username.to_string();
    let anilist_id =
        task::spawn_blocking(move || User::get_anilist_id_from_username(username.as_ref()))
            .await
            .unwrap();

    let connection = &mut database::establish_connection();

    let response = match User::get_user_by_discord_id(user.id.into(), connection) {
        Some(db_user) => {
            info!(
                "User with details: id: {}, anilist_id: {}, anilist_username: {} already exists",
                db_user.discord_id, db_user.anilist_id, db_user.anilist_username
            );
            format!(
                "Hello {}, This account is already associated with the Anilist user {}.",
                user.name, db_user.anilist_username
            )
        }
        None => {
            User::create_user(
                user.id.into(),
                anilist_id,
                anilist_username.to_owned(),
                connection,
            );

            info!(
                "Created user with details: id: {}, anilist_id: {}, anilist_username: {}",
                user.id, anilist_id, anilist_username
            );
            format!(
                "Hello {}, I have linked your Anilist account {} to your user.",
                user.name, anilist_username
            )
        }
    };
    response
}
