use crate::{
    commands::response::CommandResponse,
    models::db::user::User,
    utils::{
        database,
        privacy::{configure_sentry_scope, hash_user_id},
    },
};

use serenity::{
    all::{
        CommandDataOption, CommandDataOptionValue, CommandInteraction, CreateCommandOption,
        EditInteractionResponse,
    },
    builder::CreateCommand,
    client::Context,
    model::application::CommandOptionType,
};
use tokio::task;
use tracing::{error, instrument};

const CONFIRMATION_OPTION: &str = "confirmation";
const CONFIRM_UNREGISTER: &str = "confirm";
const CANCEL_UNREGISTER: &str = "cancel";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnregisterOutcome {
    Unlinked { username: String },
    NotLinked,
    Cancelled,
    Failed,
}

pub fn register() -> CreateCommand {
    CreateCommand::new("unregister")
        .description("Unlink your AniList account from Annie Mei")
        .add_option(
            CreateCommandOption::new(
                CommandOptionType::String,
                CONFIRMATION_OPTION,
                "Confirm whether to unlink your AniList account",
            )
            .add_string_choice("Confirm unlink", CONFIRM_UNREGISTER)
            .add_string_choice("Cancel", CANCEL_UNREGISTER)
            .required(true),
        )
}

fn parse_unregister_confirmation(options: &[CommandDataOption]) -> Option<bool> {
    options
        .iter()
        .find(|option| option.name == CONFIRMATION_OPTION)
        .and_then(|option| match &option.value {
            CommandDataOptionValue::String(value) => match value.as_str() {
                CONFIRM_UNREGISTER => Some(true),
                CANCEL_UNREGISTER => Some(false),
                _ => None,
            },
            _ => None,
        })
}

#[instrument(name = "command.unregister.handle")]
pub fn handle_unregister(outcome: UnregisterOutcome) -> CommandResponse {
    match outcome {
        UnregisterOutcome::Unlinked { username } => CommandResponse::Content(format!(
            "Your AniList account **{username}** has been unlinked from Annie Mei."
        )),
        UnregisterOutcome::NotLinked => CommandResponse::Content(
            "You do not have a linked AniList account. Run `/register` if you want to link one."
                .to_string(),
        ),
        UnregisterOutcome::Cancelled => CommandResponse::Content(
            "Cancelled. Your AniList account link was not changed.".to_string(),
        ),
        UnregisterOutcome::Failed => CommandResponse::Content(
            "I hit an internal error while unlinking your AniList account. Please try again later."
                .to_string(),
        ),
    }
}

#[instrument(name = "unregister.delete_user_registration_blocking", skip(database_pool, discord_id), fields(discord_user_id = %hash_user_id(discord_id as u64)))]
fn delete_user_registration(
    database_pool: crate::utils::database::DbPool,
    discord_id: i64,
) -> Result<Option<User>, diesel::result::Error> {
    let mut connection = database::get_connection(&database_pool);
    User::delete_user_by_discord_id(discord_id, &mut connection)
}

#[instrument(name = "command.unregister.run", skip(ctx, interaction))]
pub async fn run(ctx: &Context, interaction: &mut CommandInteraction) {
    let _ = interaction.defer_ephemeral(&ctx.http).await;

    let user = &interaction.user;
    configure_sentry_scope("Unregister", user.id.get(), None);

    let Some(confirmed) = parse_unregister_confirmation(&interaction.data.options) else {
        let builder = EditInteractionResponse::new()
            .content("Missing or invalid `confirmation` option — choose `Confirm unlink` to unlink your AniList account.");
        let _ = interaction.edit_response(&ctx.http, builder).await;
        return;
    };

    if !confirmed {
        let builder = match handle_unregister(UnregisterOutcome::Cancelled) {
            CommandResponse::Content(content) => EditInteractionResponse::new().content(content),
            CommandResponse::Embed(embed) => EditInteractionResponse::new().embed(*embed),
            CommandResponse::Message(content) => EditInteractionResponse::new().content(content),
        };
        let _ = interaction.edit_response(&ctx.http, builder).await;
        return;
    }

    let Some(database_pool) = database::get_pool_from_context(ctx).await else {
        let builder = EditInteractionResponse::new()
            .content("Database is not initialized. Please try again later.");
        let _ = interaction.edit_response(&ctx.http, builder).await;
        return;
    };

    let discord_id = user.id.get() as i64;
    let db_result =
        task::spawn_blocking(move || delete_user_registration(database_pool, discord_id)).await;

    let outcome = match db_result {
        Ok(Ok(Some(deleted_user))) => UnregisterOutcome::Unlinked {
            username: deleted_user.anilist_username,
        },
        Ok(Ok(None)) => UnregisterOutcome::NotLinked,
        Ok(Err(err)) => {
            error!(
                error = %err,
                discord_user_id = %hash_user_id(discord_id as u64),
                "Failed to delete AniList profile link from database"
            );
            UnregisterOutcome::Failed
        }
        Err(err) => {
            error!(
                error = %err,
                discord_user_id = %hash_user_id(discord_id as u64),
                "Failed to join unregister database task"
            );
            UnregisterOutcome::Failed
        }
    };

    let builder = match handle_unregister(outcome) {
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
    fn handle_unregister_with_linked_account_confirms_unlink() {
        let response = handle_unregister(UnregisterOutcome::Unlinked {
            username: "annie".to_string(),
        });

        assert!(response.is_content(), "expected Content variant");
        let content = response.unwrap_content();
        assert!(content.contains("has been unlinked"));
        assert!(content.contains("**annie**"));
    }

    #[test]
    fn handle_unregister_without_linked_account_is_user_friendly() {
        let response = handle_unregister(UnregisterOutcome::NotLinked);

        assert!(response.is_content(), "expected Content variant");
        let content = response.unwrap_content();
        assert!(content.contains("do not have a linked AniList account"));
        assert!(content.contains("/register"));
    }

    #[test]
    fn handle_unregister_failure_returns_retry_message() {
        let response = handle_unregister(UnregisterOutcome::Failed);

        assert!(response.is_content(), "expected Content variant");
        let content = response.unwrap_content();
        assert!(content.contains("internal error"));
        assert!(content.contains("try again later"));
    }

    #[test]
    fn handle_unregister_cancelled_confirms_no_change() {
        let response = handle_unregister(UnregisterOutcome::Cancelled);

        assert!(response.is_content(), "expected Content variant");
        let content = response.unwrap_content();
        assert!(content.contains("Cancelled"));
        assert!(content.contains("not changed"));
    }

    #[test]
    fn parses_confirmed_unregister_option() {
        let options: Vec<CommandDataOption> = serde_json::from_value(serde_json::json!([{
            "name": "confirmation",
            "type": 3,
            "value": "confirm"
        }]))
        .expect("options deserialize");

        assert_eq!(parse_unregister_confirmation(&options), Some(true));
    }

    #[test]
    fn parses_cancelled_unregister_option() {
        let options: Vec<CommandDataOption> = serde_json::from_value(serde_json::json!([{
            "name": "confirmation",
            "type": 3,
            "value": "cancel"
        }]))
        .expect("options deserialize");

        assert_eq!(parse_unregister_confirmation(&options), Some(false));
    }
}
