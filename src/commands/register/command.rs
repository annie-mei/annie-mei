use crate::{
    models::db::oauth_credential::OAuthCredential,
    utils::{
        database,
        oauth::{OAuthContextError, build_oauth_start_url, get_config_from_context},
        privacy::{configure_sentry_scope, hash_user_id},
    },
};

use serenity::{
    all::{CommandInteraction, CreateButton, EditInteractionResponse},
    builder::CreateCommand,
    client::Context,
    model::prelude::UserId,
};
use tokio::task;
use tracing::{error, info, instrument};

pub fn register() -> CreateCommand {
    CreateCommand::new("register")
        .description("Link or relink your AniList account with a secure OAuth flow")
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
            "Click the button below to link your AniList account. This secure link is only for you and {expires_in}. If the page says the link expired or failed, or if you ever need to reconnect AniList later, run `/register` again in Discord.",
        ),
        oauth_url: Some(oauth_url.to_string()),
    }
}

#[instrument(
    name = "command.register.handle_already_linked",
    skip(anilist_id, anilist_username)
)]
fn handle_already_linked(anilist_id: i64, anilist_username: Option<&str>) -> RegisterResponse {
    let display_name = anilist_username
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| format!("AniList account ID {anilist_id}"));
    let profile_url = anilist_username
        .map(|username| format!("https://anilist.co/user/{username}/"))
        .unwrap_or_else(|| format!("https://anilist.co/user/{anilist_id}/"));

    RegisterResponse {
        content: format!(
            "You're already linked to AniList account **{display_name}**.\nProfile: <{profile_url}>\nIf you want to link a different AniList account, run `/unregister confirmation:Confirm unlink` first, then run `/register` again."
        ),
        oauth_url: None,
    }
}

#[instrument(name = "command.register.handle_lookup_error")]
fn handle_lookup_error() -> RegisterResponse {
    RegisterResponse {
        content: "I couldn't check your existing AniList link right now. Please try `/register` again in a moment.".to_string(),
        oauth_url: None,
    }
}

#[instrument(name = "command.register.handle_error", skip(err))]
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

#[instrument(name = "register.fetch_existing_link_blocking", skip(database_pool, discord_id), fields(discord_user_id = %hash_user_id(discord_id.get())))]
fn fetch_existing_link(
    database_pool: crate::utils::database::DbPool,
    discord_id: UserId,
) -> Result<Option<OAuthCredential>, diesel::result::Error> {
    let mut connection = database::get_connection(&database_pool);
    OAuthCredential::get_by_discord_id(discord_id, &mut connection)
}

#[instrument(name = "command.register.run", skip(ctx, interaction))]
pub async fn run(ctx: &Context, interaction: &mut CommandInteraction) {
    let _ = interaction.defer_ephemeral(&ctx.http).await;

    let discord_id = interaction.user.id;
    configure_sentry_scope("Register", discord_id.get(), None);

    let Some(database_pool) = database::get_pool_from_context(ctx).await else {
        error!(
            discord_user_id = %hash_user_id(discord_id.get()),
            "Database pool not found while checking existing AniList link"
        );
        let response = handle_lookup_error();
        let builder = EditInteractionResponse::new().content(response.content);
        let _ = interaction.edit_response(&ctx.http, builder).await;
        return;
    };

    let linked_account =
        task::spawn_blocking(move || fetch_existing_link(database_pool, discord_id)).await;

    let response = match linked_account {
        Ok(Ok(Some(existing_link))) => handle_already_linked(
            existing_link.anilist_id,
            existing_link.anilist_username.as_deref(),
        ),
        Ok(Ok(None)) => match get_config_from_context(ctx).await {
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
        Ok(Err(err)) => {
            error!(
                error = %err,
                discord_user_id = %hash_user_id(discord_id.get()),
                "Failed to fetch existing AniList link from database"
            );
            handle_lookup_error()
        }
        Err(err) => {
            error!(
                error = %err,
                discord_user_id = %hash_user_id(discord_id.get()),
                "Failed to join register database task"
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
        assert!(response.content.contains("reconnect AniList later"));
    }

    #[test]
    fn register_subminute_ttl_rounds_up_to_one_minute_in_copy() {
        let response = handle_register("https://auth.example.com/oauth/anilist/start?ctx=abc", 59);

        assert!(response.content.contains("about 1 minute"));
        assert!(!response.content.contains("about 1 minutes"));
    }

    #[test]
    fn already_linked_user_does_not_receive_oauth_link() {
        let response = handle_already_linked(4567, Some("AniUser"));

        assert_eq!(response.oauth_url, None);
        assert!(response.content.contains("already linked"));
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
        let response = handle_already_linked(4567, None);

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
        assert!(response.content.contains("link your AniList account"));
    }

    #[test]
    fn lookup_failure_returns_retry_message_without_oauth_link() {
        let response = handle_lookup_error();

        assert_eq!(response.oauth_url, None);
        assert!(response.content.contains("couldn't check"));
        assert!(response.content.contains("try `/register` again"));
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
