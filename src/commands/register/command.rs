use crate::{
    models::db::oauth_credential::OAuthCredential,
    utils::{
        database::get_pool_from_context,
        oauth::{OAuthContextError, build_oauth_start_url, get_config_from_context},
        privacy::{configure_sentry_scope, hash_user_id},
    },
};

use serenity::{
    all::{CommandInteraction, CreateButton, EditInteractionResponse},
    builder::CreateCommand,
    client::Context,
};
use tracing::{error, info, instrument};

pub fn register() -> CreateCommand {
    CreateCommand::new("register").description("Link or refresh your AniList account")
}

#[derive(Debug, PartialEq, Eq)]
struct RegisterResponse {
    content: String,
    oauth_url: Option<String>,
}

#[instrument(
    name = "command.register.handle_register",
    skip(oauth_url),
    fields(ttl_seconds)
)]
fn handle_register(oauth_url: &str, ttl_seconds: i64) -> RegisterResponse {
    let ttl_minutes = (ttl_seconds + 59) / 60;
    let expires_in = if ttl_minutes == 1 {
        "expires in about 1 minute".to_string()
    } else {
        format!("expires in about {ttl_minutes} minutes")
    };

    RegisterResponse {
        content: format!(
            "Click the button below to link AniList. This secure link is only for you and {expires_in}. If it expires or you need to reconnect later, run `/register` again.",
        ),
        oauth_url: Some(oauth_url.to_string()),
    }
}

#[instrument(name = "command.register.handle_already_linked", skip(credential))]
fn handle_already_linked(credential: &OAuthCredential) -> RegisterResponse {
    RegisterResponse {
        content: format!(
            "You're linked to AniList as **{}**.\nProfile: <{}>\nTo switch accounts, run `/unregister confirmation:Confirm unlink`, then `/register` again.",
            credential.anilist_display_name(),
            credential.anilist_profile_url(),
        ),
        oauth_url: None,
    }
}

#[instrument(name = "command.register.handle_lookup_error")]
fn handle_lookup_error() -> RegisterResponse {
    RegisterResponse {
        content: "I couldn't check your AniList link right now. Try `/register` again in a moment."
            .to_string(),
        oauth_url: None,
    }
}

#[instrument(name = "command.register.handle_error", skip(err))]
fn handle_register_error(err: &OAuthContextError) -> RegisterResponse {
    let content = match err {
        OAuthContextError::MissingEnv(_) => {
            "AniList linking is not configured right now. Please try again later.".to_string()
        }
        _ => {
            "I couldn't start AniList linking right now. Please try again in a moment.".to_string()
        }
    };

    RegisterResponse {
        content,
        oauth_url: None,
    }
}

#[instrument(name = "command.register.run", skip(ctx, interaction))]
pub async fn run(ctx: &Context, interaction: &mut CommandInteraction) {
    let _ = interaction.defer_ephemeral(&ctx.http).await;

    let discord_id = interaction.user.id;
    configure_sentry_scope("Register", discord_id.get(), None);

    let Some(database_pool) = get_pool_from_context(ctx).await else {
        error!(
            discord_user_id = %hash_user_id(discord_id.get()),
            "Database pool not found while checking existing AniList link"
        );
        let response = handle_lookup_error();
        let builder = EditInteractionResponse::new().content(response.content);
        let _ = interaction.edit_response(&ctx.http, builder).await;
        return;
    };

    let linked_account = OAuthCredential::get_by_discord_id(discord_id, &database_pool).await;

    let response = match linked_account {
        Ok(Some(existing_link)) => handle_already_linked(&existing_link),
        Ok(None) => match get_config_from_context(ctx).await {
            Some(config) => {
                let guild_id = interaction.guild_id.map(|id| id.to_string());
                match build_oauth_start_url(
                    &discord_id.get().to_string(),
                    guild_id.as_deref(),
                    &interaction.id.to_string(),
                    &config,
                ) {
                    Ok(oauth_url) => {
                        info!(
                            discord_user_id = %hash_user_id(discord_id.get()),
                            has_guild_id = interaction.guild_id.is_some(),
                            ttl_seconds = config.ttl_seconds,
                            "Generated OAuth register link"
                        );
                        handle_register(oauth_url.as_ref(), config.ttl_seconds)
                    }
                    Err(err) => {
                        error!(
                            error = %err,
                            discord_user_id = %hash_user_id(discord_id.get()),
                            "Failed to build OAuth start URL"
                        );
                        handle_register_error(&err)
                    }
                }
            }
            None => {
                error!(
                    discord_user_id = %hash_user_id(discord_id.get()),
                    "OAuth configuration not found in context"
                );
                handle_register_error(&OAuthContextError::MissingEnv("OAuth config not available"))
            }
        },
        Err(err) => {
            error!(
                error = %err,
                discord_user_id = %hash_user_id(discord_id.get()),
                "Failed to fetch existing AniList link from database"
            );
            handle_lookup_error()
        }
    };

    let mut builder = EditInteractionResponse::new().content(response.content);

    if let Some(url) = response.oauth_url {
        builder = builder.button(CreateButton::new_link(url).label("Link AniList Account"));
    }

    if let Err(err) = interaction.edit_response(&ctx.http, builder).await {
        error!(
            error = %err,
            discord_user_id = %hash_user_id(discord_id.get()),
            "Failed to edit register interaction response"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::utils::oauth::OAuthContextError;

    fn oauth_credential(anilist_username: Option<&str>) -> OAuthCredential {
        OAuthCredential {
            discord_user_id: "123456789".to_string(),
            anilist_id: 4567,
            anilist_username: anilist_username.map(str::to_owned),
        }
    }

    #[test]
    fn register_happy_path_returns_oauth_link() {
        let response = handle_register("https://auth.example.com/oauth/anilist/start?ctx=abc", 300);

        assert_eq!(
            response.oauth_url,
            Some("https://auth.example.com/oauth/anilist/start?ctx=abc".to_string())
        );
        assert!(response.content.contains("button below"));
        assert!(response.content.contains("only for you"));
        assert!(response.content.contains("about 5 minutes"));
        assert!(response.content.contains("run `/register` again"));
        assert!(response.content.contains("reconnect later"));
    }

    #[test]
    fn register_subminute_ttl_rounds_up_to_one_minute_in_copy() {
        let response = handle_register("https://auth.example.com/oauth/anilist/start?ctx=abc", 59);

        assert!(response.content.contains("about 1 minute"));
        assert!(!response.content.contains("about 1 minutes"));
    }

    #[test]
    fn already_linked_user_does_not_receive_oauth_link() {
        let credential = oauth_credential(Some("AniUser"));
        let response = handle_already_linked(&credential);

        assert_eq!(response.oauth_url, None);
        assert!(response.content.contains("You're linked"));
        assert!(response.content.contains("**AniUser**"));
        assert!(
            response
                .content
                .contains("https://anilist.co/user/AniUser/")
        );
        assert!(response.content.contains("/unregister"));
        assert!(response.content.contains("/register"));
    }

    #[test]
    fn already_linked_user_without_username_falls_back_to_anilist_id() {
        let credential = oauth_credential(None);
        let response = handle_already_linked(&credential);

        assert_eq!(response.oauth_url, None);
        assert!(response.content.contains("**AniList account ID 4567**"));
        assert!(response.content.contains("https://anilist.co/user/4567/"));
    }

    #[test]
    fn unregistered_user_still_receives_oauth_link() {
        let response = handle_register("https://auth.example.com/oauth/anilist/start?ctx=abc", 300);

        assert_eq!(
            response.oauth_url,
            Some("https://auth.example.com/oauth/anilist/start?ctx=abc".to_string())
        );
        assert!(response.content.contains("link AniList"));
    }

    #[test]
    fn lookup_failure_returns_retry_message_without_oauth_link() {
        let response = handle_lookup_error();

        assert_eq!(response.oauth_url, None);
        assert!(response.content.contains("couldn't check"));
        assert!(response.content.contains("Try `/register` again"));
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
