use crate::utils::privacy::configure_sentry_scope;

use serenity::{
    all::{
        CommandInteraction, CreateAttachment, CreateEmbed, CreateEmbedFooter,
        CreateInteractionResponse, CreateInteractionResponseMessage,
    },
    builder::CreateCommand,
    prelude::*,
};
use tracing::{error, instrument, warn};

pub fn register() -> CreateCommand {
    CreateCommand::new("help").description("Shows how to use the bot")
}

#[instrument(name = "command.help.run", skip(ctx, interaction))]
pub async fn run(ctx: &Context, interaction: &CommandInteraction) {
    let user = &interaction.user;

    configure_sentry_scope("Help", user.id.get(), None);

    let mut embed = CreateEmbed::new()
        .colour(0x00ff00)
        .title(format!("{} • Annie Mei Help", user.name))
        .description(
            "I can help you look up anime and manga details, theme songs, and show what guild members are watching or reading.",
        )
        .field(
            "Get started",
            "1. Run `/register` and click the secure AniList link button\n2. Finish the AniList authorization in your browser, then return to Discord\n3. Use `/anime` or `/manga` with an AniList ID or search term\n4. Use `/songs` to fetch openings/endings and links\n5. If your AniList connection ever expires later, run `/register` again to relink it",
            false,
        )
        .field(
            "Commands",
            "`/anime search:<term or id>` - anime details\n`/manga search:<term or id>` - manga details\n`/songs search:<term or id>` - OP/ED songs + links\n`/register` - open the AniList OAuth link or relink flow\n`/whoami` - show your linked AniList username and profile link\n`/ping` - bot health check\n`/help` - show this guide",
            false,
        )
        .field(
            "Tips",
            "You can search with full names, short names, or AniList IDs. If the AniList link page expires or fails, or if you need to reconnect your AniList account later, run `/register` again to get a fresh secure link.",
            false,
        )
        .footer(CreateEmbedFooter::new("Annie Mei"))
        .timestamp(chrono::Utc::now());

    let mut response_message = CreateInteractionResponseMessage::new();

    match CreateAttachment::path("./mei.jpg").await {
        Ok(attachment) => {
            embed = embed.thumbnail("attachment://mei.jpg");
            response_message = response_message.add_file(attachment);
        }
        Err(error) => warn!(error = %error, "Failed to attach help image"),
    }

    response_message = response_message.embed(embed);

    let response = CreateInteractionResponse::Message(response_message);

    if let Err(error) = interaction.create_response(&ctx.http, response).await {
        error!(
            error = %error,
            interaction_id = ?interaction.id,
            command = "help",
            "Failed to create interaction response"
        );
    }
}
