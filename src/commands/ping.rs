use serenity::{
    all::{CommandInteraction, CreateInteractionResponse, CreateInteractionResponseMessage},
    builder::CreateCommand,
    prelude::*,
};

pub fn register() -> CreateCommand {
    CreateCommand::new("ping").description("A ping command")
}

pub async fn run(ctx: &Context, interaction: &CommandInteraction) {
    let user = &interaction.user;

    sentry::configure_scope(|scope| {
        let mut context = std::collections::BTreeMap::new();
        context.insert("Command".to_string(), "Register".into());
        scope.set_context("Ping", sentry::protocol::Context::Other(context));
        scope.set_user(Some(sentry::User {
            username: Some(user.name.to_string()),
            ..Default::default()
        }));
    });

    let response_message = CreateInteractionResponseMessage::new().content(format!(
        "Hello {}! I'm Annie Mei, a bot that helps you find anime and manga!",
        user.mention()
    ));
    let response = CreateInteractionResponse::Message(response_message);

    let _ = interaction.create_response(&ctx.http, response).await;
}
