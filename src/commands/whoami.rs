use crate::{
    commands::response::CommandResponse,
    models::db::user::User,
    utils::{
        database,
        privacy::{configure_sentry_scope, hash_user_id},
    },
};

use serenity::{
    all::{CommandInteraction, EditInteractionResponse},
    builder::CreateCommand,
    client::Context,
};
use tokio::task;
use tracing::{error, instrument};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkedAniListProfile {
    pub username: String,
}

impl LinkedAniListProfile {
    fn profile_url(&self) -> String {
        format!("https://anilist.co/user/{}", self.username)
    }
}

pub fn register() -> CreateCommand {
    CreateCommand::new("whoami").description("Show your currently linked AniList account")
}

#[instrument(name = "command.whoami.handle", skip(profile))]
pub fn handle_whoami(profile: Option<LinkedAniListProfile>) -> CommandResponse {
    match profile {
        Some(profile) => CommandResponse::Content(format!(
            "Your linked AniList account is **{}**.\nProfile: <{}>",
            profile.username,
            profile.profile_url()
        )),
        None => CommandResponse::Content(
            "You have not linked an AniList account yet. Run `/register` first.".to_string(),
        ),
    }
}

#[instrument(name = "whoami.fetch_profile_blocking", skip(database_pool), fields(discord_user_id = %hash_user_id(discord_id as u64)))]
fn fetch_whoami_profile(
    database_pool: crate::utils::database::DbPool,
    discord_id: i64,
) -> Result<Option<User>, diesel::result::Error> {
    let mut connection = database::get_connection(&database_pool);
    User::get_user_by_discord_id(discord_id, &mut connection)
}

#[instrument(name = "command.whoami.run", skip(ctx, interaction))]
pub async fn run(ctx: &Context, interaction: &mut CommandInteraction) {
    let _ = interaction.defer_ephemeral(&ctx.http).await;

    let user = &interaction.user;
    configure_sentry_scope("WhoAmI", user.id.get(), None);

    let Some(database_pool) = database::get_pool_from_context(ctx).await else {
        let builder = EditInteractionResponse::new()
            .content("Database is not initialized. Please try again later.");
        let _ = interaction.edit_response(&ctx.http, builder).await;
        return;
    };

    let discord_id = user.id.get() as i64;
    let db_pool = database_pool.clone();
    let db_result = task::spawn_blocking(move || fetch_whoami_profile(db_pool, discord_id)).await;

    let response = match db_result {
        Ok(Ok(profile)) => handle_whoami(profile.map(|entry| LinkedAniListProfile {
            username: entry.anilist_username,
        })),
        Ok(Err(err)) => {
            error!(
                error = %err,
                discord_user_id = %hash_user_id(discord_id as u64),
                "Failed to fetch whoami profile from database"
            );
            CommandResponse::Content(
                "I hit an internal error while looking up your AniList account. Please try again later."
                    .to_string(),
            )
        }
        Err(err) => {
            error!(
                error = %err,
                discord_user_id = %hash_user_id(discord_id as u64),
                "Failed to join whoami database task"
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

    #[test]
    fn handle_whoami_with_linked_profile_returns_username_and_url() {
        let response = handle_whoami(Some(LinkedAniListProfile {
            username: "annie".to_string(),
        }));

        assert!(response.is_content(), "expected Content variant");
        let content = response.unwrap_content();
        assert!(
            content.contains("**annie**"),
            "expected username in response"
        );
        assert!(
            content.contains("https://anilist.co/user/annie"),
            "expected AniList profile URL in response"
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
