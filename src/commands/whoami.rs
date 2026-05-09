use crate::{
    commands::response::CommandResponse,
    models::db::oauth_credential::OAuthCredential,
    utils::{
        database::get_pool_from_context,
        privacy::{configure_sentry_scope, hash_user_id},
    },
};

use serenity::{
    all::{CommandInteraction, EditInteractionResponse},
    builder::CreateCommand,
    client::Context,
};
use tracing::{error, instrument};

pub fn register() -> CreateCommand {
    CreateCommand::new("whoami").description("Show your currently linked AniList account")
}

#[instrument(name = "command.whoami.handle", skip(profile))]
pub fn handle_whoami(profile: Option<OAuthCredential>) -> CommandResponse {
    match profile {
        Some(profile) => CommandResponse::Content(format!(
            "Your linked AniList account is **{}**.\nProfile: <{}>",
            profile.anilist_display_name(),
            profile.anilist_profile_url()
        )),
        None => CommandResponse::Content(
            "You have not linked an AniList account yet. Run `/register` first.".to_string(),
        ),
    }
}

#[instrument(name = "command.whoami.run", skip(ctx, interaction))]
pub async fn run(ctx: &Context, interaction: &mut CommandInteraction) {
    let _ = interaction.defer_ephemeral(&ctx.http).await;

    let user = &interaction.user;
    configure_sentry_scope("WhoAmI", user.id.get(), None);

    let Some(database_pool) = get_pool_from_context(ctx).await else {
        let builder = EditInteractionResponse::new()
            .content("Database is not initialized. Please try again later.");
        let _ = interaction.edit_response(&ctx.http, builder).await;
        return;
    };

    let discord_id = user.id;
    let db_result = OAuthCredential::get_by_discord_id(discord_id, &database_pool).await;

    let response = match db_result {
        Ok(profile) => handle_whoami(profile),
        Err(err) => {
            error!(
                error = %err,
                discord_user_id = %hash_user_id(discord_id.get()),
                "Failed to fetch whoami profile from database"
            );
            CommandResponse::Content(
                "I hit an internal error while looking up your AniList account. Please try again later."
                    .to_string(),
            )
        }
    };

    let builder = match response {
        CommandResponse::Content(content) => EditInteractionResponse::new().content(content),
        CommandResponse::Embed(embed) => EditInteractionResponse::new().embed(*embed),
        CommandResponse::Message(content) => EditInteractionResponse::new().content(content),
    };

    let _ = interaction.edit_response(&ctx.http, builder).await;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn oauth_credential(anilist_username: Option<&str>) -> OAuthCredential {
        OAuthCredential {
            discord_user_id: "123456789".to_string(),
            anilist_id: 4567,
            anilist_username: anilist_username.map(str::to_owned),
        }
    }

    #[test]
    fn handle_whoami_with_linked_profile_returns_anilist_username_and_url() {
        let response = handle_whoami(Some(oauth_credential(Some("AniUser"))));

        assert!(response.is_content(), "expected Content variant");
        let content = response.unwrap_content();
        assert!(
            content.contains("**AniUser**"),
            "expected AniList username in response"
        );
        assert!(
            content.contains("https://anilist.co/user/AniUser/"),
            "expected AniList profile URL in response"
        );
    }

    #[test]
    fn handle_whoami_without_username_falls_back_to_anilist_id() {
        let response = handle_whoami(Some(oauth_credential(None)));

        assert!(response.is_content(), "expected Content variant");
        let content = response.unwrap_content();
        assert!(
            content.contains("**AniList account ID 4567**"),
            "expected AniList ID fallback in response"
        );
        assert!(
            content.contains("https://anilist.co/user/4567/"),
            "expected numeric AniList profile URL fallback in response"
        );
    }

    #[test]
    fn handle_whoami_without_linked_profile_returns_register_guidance() {
        let response = handle_whoami(None);

        assert!(response.is_content(), "expected Content variant");
        let content = response.unwrap_content();
        assert!(
            content.contains("/register"),
            "expected /register guidance for unlinked users"
        );
    }
}
