use crate::utils::privacy::configure_sentry_scope;

use super::response::CommandResponse;

use serenity::{
    all::{CommandInteraction, CreateInteractionResponse, CreateInteractionResponseMessage},
    builder::CreateCommand,
    prelude::*,
};

pub fn register() -> CreateCommand {
    CreateCommand::new("ping").description("A ping command")
}

// ── Core logic (transport-agnostic) ─────────────────────────────────────

/// Produce the `/ping` response for the given user mention string.
///
/// This is the testable entry-point — it never touches `Context` or
/// `CommandInteraction`.
pub fn handle_ping(user_mention: &str) -> CommandResponse {
    CommandResponse::Message(format!(
        "Hello {user_mention}! I'm Annie Mei, a bot that helps you find anime and manga!",
    ))
}

// ── Serenity adapter (thin wrapper) ─────────────────────────────────────

pub async fn run(ctx: &Context, interaction: &CommandInteraction) {
    let user = &interaction.user;

    configure_sentry_scope("Ping", user.id.get(), None);

    let reply = handle_ping(&user.mention().to_string());

    // `handle_ping` always returns `Message`, so this branch is safe.
    let text = match reply {
        CommandResponse::Message(text) => text,
        _ => unreachable!("/ping always returns Message"),
    };

    let response_message = CreateInteractionResponseMessage::new().content(text);
    let response = CreateInteractionResponse::Message(response_message);

    let _ = interaction.create_response(&ctx.http, response).await;
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ping_happy_path_returns_message_with_greeting() {
        let response = handle_ping("<@123456>");

        assert!(response.is_message(), "expected Message variant");
        let text = response.unwrap_message();
        assert!(
            text.contains("<@123456>"),
            "response should mention the user"
        );
        assert!(
            text.contains("Annie Mei"),
            "response should mention the bot name"
        );
    }

    #[test]
    fn ping_response_includes_bot_description() {
        let text = handle_ping("<@999>").unwrap_message();
        assert!(
            text.contains("anime and manga"),
            "response should describe what the bot does"
        );
    }
}
