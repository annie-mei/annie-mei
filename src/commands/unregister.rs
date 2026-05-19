use crate::{
    commands::response::CommandResponse,
    utils::{
        database::get_pool_from_context,
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
use sqlx::PgConnection;
use tracing::{error, instrument};

const CONFIRMATION_OPTION: &str = "confirmation";
const CONFIRM_UNREGISTER: &str = "confirm";
const CANCEL_UNREGISTER: &str = "cancel";
const DELETE_OAUTH_CREDENTIALS_SQL: &str =
    "DELETE FROM annie_auth.oauth_credentials WHERE discord_user_id = $1";
const DELETE_OAUTH_SESSIONS_SQL: &str =
    "DELETE FROM annie_auth.oauth_sessions WHERE discord_user_id = $1";
const OAUTH_CREDENTIALS_TABLE: &str = "annie_auth.oauth_credentials";
const OAUTH_SESSIONS_TABLE: &str = "annie_auth.oauth_sessions";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnregisterOutcome {
    Unlinked,
    NotLinked,
    Cancelled,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DeletedRegistrations {
    oauth_credentials_deleted: u64,
    oauth_sessions_deleted: u64,
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

#[instrument(name = "command.unregister.parse_confirmation", skip(options))]
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

#[instrument(name = "command.unregister.handle", skip(outcome))]
pub fn handle_unregister(outcome: UnregisterOutcome) -> CommandResponse {
    match outcome {
        UnregisterOutcome::Unlinked => CommandResponse::Content(
            "Your AniList account has been unlinked from Annie Mei and your stored OAuth credentials have been deleted."
                .to_string(),
        ),
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

#[instrument(name = "unregister.delete_user_registration_transaction", skip(conn, discord_id), fields(discord_user_id = %hash_user_id(discord_id)))]
async fn delete_user_registration_in_transaction(
    discord_id: u64,
    conn: &mut PgConnection,
) -> Result<DeletedRegistrations, sqlx::Error> {
    let discord_id_str = discord_id.to_string();

    let oauth_credentials_deleted = delete_auth_records(
        OAUTH_CREDENTIALS_TABLE,
        DELETE_OAUTH_CREDENTIALS_SQL,
        &discord_id_str,
        conn,
    )
    .await?;

    let oauth_sessions_deleted = delete_auth_records(
        OAUTH_SESSIONS_TABLE,
        DELETE_OAUTH_SESSIONS_SQL,
        &discord_id_str,
        conn,
    )
    .await?;

    Ok(DeletedRegistrations {
        oauth_credentials_deleted,
        oauth_sessions_deleted,
    })
}

#[instrument(name = "unregister.delete_auth_records", skip(conn, sql, discord_id), fields(table = _table))]
async fn delete_auth_records(
    _table: &str,
    sql: &str,
    discord_id: &str,
    conn: &mut PgConnection,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(sql).bind(discord_id).execute(conn).await?;
    Ok(result.rows_affected())
}

#[instrument(name = "command.unregister.outcome_from_deletions", skip(deletions))]
fn outcome_from_deletions(deletions: DeletedRegistrations) -> UnregisterOutcome {
    // An oauth_sessions row by itself only means the user started but never
    // completed an OAuth flow — they were never actually linked. Reporting
    // `Unlinked` in that case would tell them "your AniList account has been
    // unlinked … and your stored OAuth credentials have been deleted", which
    // is misleading. Only an oauth_credentials row counts as having been
    // linked; orphaned sessions are silently cleaned up alongside.
    if deletions.oauth_credentials_deleted > 0 {
        UnregisterOutcome::Unlinked
    } else {
        UnregisterOutcome::NotLinked
    }
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

    let Some(database_pool) = get_pool_from_context(ctx).await else {
        let builder = EditInteractionResponse::new()
            .content("Database is not initialized. Please try again later.");
        let _ = interaction.edit_response(&ctx.http, builder).await;
        return;
    };

    let discord_id = user.id.get();

    // Start a transaction and delete both records
    let mut transaction = match database_pool.begin().await {
        Ok(tx) => tx,
        Err(err) => {
            error!(
                error = %err,
                discord_user_id = %hash_user_id(discord_id),
                "Failed to begin unregister transaction"
            );
            let builder = match handle_unregister(UnregisterOutcome::Failed) {
                CommandResponse::Content(content) => {
                    EditInteractionResponse::new().content(content)
                }
                CommandResponse::Embed(embed) => EditInteractionResponse::new().embed(*embed),
                CommandResponse::Message(content) => {
                    EditInteractionResponse::new().content(content)
                }
            };
            let _ = interaction.edit_response(&ctx.http, builder).await;
            return;
        }
    };

    let outcome = match delete_user_registration_in_transaction(discord_id, &mut transaction).await
    {
        Ok(deletions) => {
            if let Err(err) = transaction.commit().await {
                error!(
                    error = %err,
                    discord_user_id = %hash_user_id(discord_id),
                    "Failed to commit unregister transaction"
                );
                UnregisterOutcome::Failed
            } else {
                outcome_from_deletions(deletions)
            }
        }
        Err(err) => {
            error!(
                error = %err,
                discord_user_id = %hash_user_id(discord_id),
                "Failed to delete AniList profile link from database"
            );
            // Transaction will be automatically rolled back when dropped
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
    fn handle_unregister_with_unlinked_outcome_confirms_cleanup() {
        let response = handle_unregister(UnregisterOutcome::Unlinked);

        assert!(response.is_content(), "expected Content variant");
        let content = response.unwrap_content();
        assert!(content.contains("has been unlinked"));
        assert!(content.contains("OAuth credentials have been deleted"));
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
    fn deletion_outcome_reports_unlinked_when_any_credential_row_was_deleted() {
        let outcome = outcome_from_deletions(DeletedRegistrations {
            oauth_credentials_deleted: 1,
            oauth_sessions_deleted: 0,
        });
        assert_eq!(outcome, UnregisterOutcome::Unlinked);
    }

    #[test]
    fn deletion_outcome_reports_not_linked_when_only_in_flight_sessions_were_deleted() {
        // An orphaned oauth_sessions row means the user started but never
        // completed /register. Reporting Unlinked here would falsely claim
        // they had a link in the first place — they didn't.
        let outcome = outcome_from_deletions(DeletedRegistrations {
            oauth_credentials_deleted: 0,
            oauth_sessions_deleted: 1,
        });
        assert_eq!(outcome, UnregisterOutcome::NotLinked);
    }

    #[test]
    fn deletion_outcome_reports_unlinked_when_credentials_and_sessions_were_deleted() {
        let outcome = outcome_from_deletions(DeletedRegistrations {
            oauth_credentials_deleted: 1,
            oauth_sessions_deleted: 1,
        });
        assert_eq!(outcome, UnregisterOutcome::Unlinked);
    }

    #[test]
    fn deletion_outcome_reports_not_linked_when_nothing_was_deleted() {
        let outcome = outcome_from_deletions(DeletedRegistrations {
            oauth_credentials_deleted: 0,
            oauth_sessions_deleted: 0,
        });
        assert_eq!(outcome, UnregisterOutcome::NotLinked);
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
