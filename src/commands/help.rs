use crate::utils::privacy::configure_sentry_scope;

use serenity::{
    all::{
        CommandInteraction, CreateAttachment, CreateEmbed, CreateEmbedFooter,
        CreateInteractionResponse, CreateInteractionResponseMessage, Timestamp,
    },
    builder::CreateCommand,
    prelude::*,
};
use tracing::{error, instrument, warn};

pub fn register() -> CreateCommand {
    CreateCommand::new("help").description("Show Annie Mei commands and tips")
}

#[instrument(name = "command.help.run", skip(ctx, interaction))]
pub async fn run(ctx: &Context, interaction: &CommandInteraction) {
    let user = &interaction.user;

    configure_sentry_scope("Help", user.id.get(), None);

    let mut embed = CreateEmbed::new()
        .colour(0x00ff00)
        .title(format!("{} • Annie Mei Help", user.name))
        .description(
            "I can look up anime, manga, characters, recommendations, theme songs, and what people in this server are watching or reading.",
        )
        .field(
            "Get started",
            "1. Run `/register` and click the secure AniList link button\n2. Finish authorization in your browser, then return to Discord\n3. Use `/anime`, `/manga`, `/recommend`, or `/character` with an AniList ID or search term\n4. Use `/search` when you only remember a vibe, plot, or partial title\n5. Use `/songs` for opening and ending theme songs",
            false,
        )
        .field(
            "Commands",
            "`/anime search:<term or id>` - anime details\n`/manga search:<term or id>` - manga details\n`/search query:<description>` - natural-language anime/manga search\n`/recommend type:<anime|manga> search:<term or id>` - community recommendations\n`/character search:<term or id> spoilers:<allow|disallow>` - character details\n`/songs search:<term or id>` - opening and ending themes\n`/settings` - preferences for titles, analytics, and guild scores\n`/register` - link or relink AniList\n`/unregister confirmation:<confirm|cancel>` - unlink AniList\n`/whoami` - show your linked AniList account\n`/ping` - bot health check\n`/help` - show this guide",
            false,
        )
        .field(
            "Tips",
            "Full titles, short titles, and AniList IDs all work. If you only remember a scene or premise, try `/search`. Run `/settings` to pick title language and privacy preferences. If your AniList link expires or you want to reconnect, run `/register` again.",
            false,
        )
        .footer(CreateEmbedFooter::new("Annie Mei"))
        .timestamp(Timestamp::now());

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
