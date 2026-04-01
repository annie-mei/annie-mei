use crate::utils::{
    oauth::{OAuthContextError, build_oauth_start_url, load_context_config},
    privacy::{configure_sentry_scope, hash_user_id},
};

use serenity::{
    all::{
        CommandInteraction, CreateButton, CreateInteractionResponse,
        CreateInteractionResponseMessage,
    },
    builder::CreateCommand,
    client::Context,
};
use tracing::{error, info, instrument};

pub fn register() -> CreateCommand {
    CreateCommand::new("register").description("Link your AniList account with a secure OAuth flow")
}

#[derive(Debug, PartialEq, Eq)]
struct RegisterResponse {
    content: String,
    oauth_url: Option<String>,
}

fn handle_register(oauth_url: &str, ttl_seconds: i64) -> RegisterResponse {
    let ttl_minutes = ttl_seconds / 60;

    RegisterResponse {
        content: format!(
            "Click the button below to link your AniList account. This secure link is only for you and expires in about {ttl_minutes} minutes. If the page says the link expired or failed, run `/register` again in Discord.",
        ),
        oauth_url: Some(oauth_url.to_string()),
    }
}

fn handle_register_error(err: &OAuthContextError) -> RegisterResponse {
    let content = match err {
        OAuthContextError::MissingEnv(_) => {
            "AniList account linking is not configured right now. Please try again later."
                .to_string()
        }
        _ => "I couldn't start the AniList linking flow right now. Please try again in a moment."
            .to_string(),
    };

    RegisterResponse {
        content,
        oauth_url: None,
    }
}

#[instrument(name = "command.register.run", skip(ctx, interaction))]
pub async fn run(ctx: &Context, interaction: &CommandInteraction) {
    let user = &interaction.user;
    configure_sentry_scope("Register", user.id.get(), None);

    let response = match load_context_config().and_then(|config| {
        let guild_id = interaction.guild_id.map(|id| id.to_string());
        let oauth_url = build_oauth_start_url(
            &user.id.get().to_string(),
            guild_id.as_deref(),
            &interaction.id.to_string(),
            &config,
        )?;

        info!(
            discord_user_id = %hash_user_id(user.id.get()),
            has_guild_id = interaction.guild_id.is_some(),
            ttl_seconds = config.ttl_seconds,
            "Generated OAuth register link"
        );

        Ok(handle_register(oauth_url.as_ref(), config.ttl_seconds))
    }) {
        Ok(response) => response,
        Err(err) => {
            error!(
                error = %err,
                discord_user_id = %hash_user_id(user.id.get()),
                "Failed to prepare AniList OAuth register flow"
            );
            handle_register_error(&err)
        }
    };

    let mut message = CreateInteractionResponseMessage::new()
        .ephemeral(true)
        .content(response.content);

    if let Some(url) = response.oauth_url {
        message = message.button(CreateButton::new_link(url).label("Link AniList Account"));
    }

    let response = CreateInteractionResponse::Message(message);

    if let Err(err) = interaction.create_response(&ctx.http, response).await {
        error!(
            error = %err,
            discord_user_id = %hash_user_id(user.id.get()),
            "Failed to create register interaction response"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::utils::oauth::OAuthContextError;

    #[test]
    fn register_happy_path_returns_oauth_link() {
        let response = handle_register("https://auth.example.com/oauth/anilist/start?ctx=abc", 300);

        assert_eq!(
            response.oauth_url,
            Some("https://auth.example.com/oauth/anilist/start?ctx=abc".to_string())
        );
        assert!(response.content.contains("button below"));
        assert!(response.content.contains("only for you"));
        assert!(response.content.contains("5 minutes"));
        assert!(response.content.contains("run `/register` again"));
    }

    #[test]
    fn register_missing_config_returns_user_facing_message() {
        let response =
            handle_register_error(&OAuthContextError::MissingEnv("AUTH_SERVICE_BASE_URL"));

        assert_eq!(response.oauth_url, None);
        assert!(response.content.contains("not configured"));
    }

    #[test]
    fn register_other_errors_return_retry_message() {
        let response = handle_register_error(&OAuthContextError::InvalidTtl("0".to_string()));

        assert_eq!(response.oauth_url, None);
        assert!(response.content.contains("couldn't start"));
        assert!(response.content.contains("try again"));
    }
}
