use crate::utils::privacy::configure_sentry_scope;

use serenity::{
    all::{
        CommandInteraction, CreateAttachment, CreateEmbed, CreateEmbedFooter,
        CreateInteractionResponse, CreateInteractionResponseMessage,
    },
    builder::CreateCommand,
    prelude::*,
};
use tracing::warn;

pub fn register() -> CreateCommand {
    CreateCommand::new("help").description("Shows how to use the bot")
}

pub async fn run(ctx: &Context, interaction: &CommandInteraction) {
    let user = &interaction.user;

    configure_sentry_scope("Help", user.id.get(), None);

    let embed = CreateEmbed::new()
        .colour(0x00ff00)
        .title(format!("{} • Annie Mei Help", user.mention()))
        .description(
            "I can help you look up anime and manga details, theme songs, and show what guild members are watching or reading.",
        )
        .field(
            "Get started",
            "1. Run `/register anilist:<username>` to link your AniList account\n2. Use `/anime` or `/manga` with an AniList ID or search term\n3. Use `/songs` to fetch openings/endings and links",
            false,
        )
        .field(
            "Commands",
            "`/anime search:<term or id>` - anime details\n`/manga search:<term or id>` - manga details\n`/songs search:<term or id>` - OP/ED songs + links\n`/register anilist:<username>` - link your AniList profile\n`/ping` - bot health check\n`/help` - show this guide",
            false,
        )
        .field(
            "Tips",
            "You can search with full names, short names, or AniList IDs. If no result is found, try a more specific query.",
            false,
        )
        .footer(CreateEmbedFooter::new("Annie Mei"))
        .timestamp(chrono::Utc::now())
        .thumbnail("attachment://mei.jpg");

    let mut response_message = CreateInteractionResponseMessage::new().embed(embed);

    match CreateAttachment::path("./mei.jpg").await {
        Ok(attachment) => {
            response_message = response_message.add_file(attachment);
        }
        Err(error) => warn!(error = %error, "Failed to attach help image"),
    }

    let response = CreateInteractionResponse::Message(response_message);

    let _ = interaction.create_response(&ctx.http, response).await;
}
