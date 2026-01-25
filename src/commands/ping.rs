use crate::utils::privacy::configure_sentry_scope;

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

    configure_sentry_scope("Ping", user.id.get(), None);

    let response_message = CreateInteractionResponseMessage::new().content(format!(
        "Hello {}! I'm Annie Mei, a bot that helps you find anime and manga!",
        user.mention()
    ));
    let response = CreateInteractionResponse::Message(response_message);

    let _ = interaction.create_response(&ctx.http, response).await;
}
