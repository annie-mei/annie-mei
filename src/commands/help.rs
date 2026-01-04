use serenity::{
    all::{
        CommandInteraction, CreateAttachment, CreateEmbed, CreateEmbedFooter,
        CreateInteractionResponse, CreateInteractionResponseMessage,
    },
    builder::CreateCommand,
    prelude::*,
};

pub fn register() -> CreateCommand {
    CreateCommand::new("help").description("Shows how to use the bot")
}

pub async fn run(ctx: &Context, interaction: &CommandInteraction) {
    let user = &interaction.user;

    sentry::configure_scope(|scope| {
        let mut context = std::collections::BTreeMap::new();
        context.insert("Command".to_string(), "Register".into());
        scope.set_context("Help", sentry::protocol::Context::Other(context));
        scope.set_user(Some(sentry::User {
            username: Some(user.name.to_string()),
            ..Default::default()
        }));
    });

    let embed = CreateEmbed::new()
        .colour(0x00ff00)
        .title(format!("Hello there {}!", user.mention()))
        .description("Use these commands to interact with Annie Mei!")
        .field("/anime", "Search for an anime", false)
        .field("/manga", "Search for a manga", false)
        .field("/songs", "Lookup an anime's songs", false)
        .field("/register", "Tell me your Anilist username", false)
        .field("/help", "Show this message", false)
        .field("/ping", "Check if I'm reachable", false)
        .footer(CreateEmbedFooter::new("Annie Mei"))
        .timestamp(chrono::Utc::now())
        .thumbnail("attachment://mei.jpg");

    let attachment = CreateAttachment::path("./mei.jpg").await.unwrap();

    let response_message = CreateInteractionResponseMessage::new()
        .embed(embed)
        .add_file(attachment);
    let response = CreateInteractionResponse::Message(response_message);

    let _ = interaction.create_response(&ctx.http, response).await;
}
