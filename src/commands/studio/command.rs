use crate::{
    commands::{
        input_validation::validate_search_term,
        response::CommandResponse,
        studio::fetcher::{StudioFetchError, fetch_studio},
    },
    models::anilist_studio::Studio,
    utils::privacy::configure_sentry_scope,
};

use serde_json::json;
use serenity::{
    all::{
        CommandDataOption, CommandDataOptionValue, CommandInteraction, CreateCommandOption,
        EditInteractionResponse,
    },
    builder::CreateCommand,
    client::Context,
    model::application::CommandOptionType,
};
use tracing::{error, info, instrument};

const SEARCH_OPTION: &str = "search";
const NOT_FOUND_STUDIO: &str = "I couldn't find that studio on AniList.";
const STUDIO_LOOKUP_ERROR: &str =
    "I couldn't reach AniList to look up that studio. Please try again shortly.";

pub fn register() -> CreateCommand {
    CreateCommand::new("studio")
        .description("Look up an anime production studio on AniList")
        .add_option(
            CreateCommandOption::new(
                CommandOptionType::String,
                SEARCH_OPTION,
                "AniList studio ID or studio name",
            )
            .required(true),
        )
}

#[instrument(name = "command.studio.parse_options", skip(options))]
fn parse_studio_options(options: &[CommandDataOption]) -> Option<String> {
    options
        .iter()
        .find(|option| option.name == SEARCH_OPTION)
        .and_then(|option| match &option.value {
            CommandDataOptionValue::String(search_term) => Some(search_term.clone()),
            _ => None,
        })
}

pub fn handle_studio(result: Result<Option<Studio>, StudioFetchError>) -> CommandResponse {
    match result {
        Ok(Some(studio)) => CommandResponse::Embed(Box::new(studio.transform_response_embed())),
        Ok(None) => CommandResponse::Content(NOT_FOUND_STUDIO.to_string()),
        Err(error) => {
            error!(error = %error, "Studio lookup failed");
            CommandResponse::Content(STUDIO_LOOKUP_ERROR.to_string())
        }
    }
}

#[instrument(name = "command.studio.run", skip(ctx, interaction))]
pub async fn run(ctx: &Context, interaction: &mut CommandInteraction) {
    let _ = interaction.defer(&ctx.http).await;

    let Some(search_term) = parse_studio_options(&interaction.data.options) else {
        let builder = EditInteractionResponse::new()
            .content("Tell me which studio to look up with `search:<name or AniList ID>`.");
        let _ = interaction.edit_response(&ctx.http, builder).await;
        return;
    };

    if let Err(error) = validate_search_term(&search_term) {
        let builder = EditInteractionResponse::new().content(format!(
            "I couldn't use that search: {error}. Try a studio name or AniList ID."
        ));
        let _ = interaction.edit_response(&ctx.http, builder).await;
        return;
    }

    configure_sentry_scope(
        "Studio",
        interaction.user.id.get(),
        Some(json!(search_term)),
    );
    info!(search_len = search_term.len(), "Got command 'studio'");

    let response = handle_studio(fetch_studio(&search_term).await);
    let result = match response {
        CommandResponse::Content(text) | CommandResponse::Message(text) => {
            interaction
                .edit_response(&ctx.http, EditInteractionResponse::new().content(text))
                .await
        }
        CommandResponse::Embed(embed) => {
            interaction
                .edit_response(&ctx.http, EditInteractionResponse::new().embed(*embed))
                .await
        }
    };

    if let Err(error) = result {
        error!(error = %error, "Failed to edit studio command response");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_studio() -> Studio {
        serde_json::from_value(serde_json::json!({
            "id": 2,
            "name": "Kyoto Animation",
            "isAnimationStudio": true,
            "favourites": 1234,
            "siteUrl": "https://anilist.co/studio/2",
            "media": { "nodes": [] }
        }))
        .expect("studio fixture should deserialize")
    }

    #[test]
    fn parses_required_search_option() {
        let options: Vec<CommandDataOption> = serde_json::from_value(serde_json::json!([{
            "name": "search",
            "type": 3,
            "value": "Kyoto Animation"
        }]))
        .expect("options should deserialize");

        assert_eq!(
            parse_studio_options(&options),
            Some("Kyoto Animation".to_string())
        );
    }

    #[test]
    fn successful_lookup_returns_embed() {
        let response = handle_studio(Ok(Some(sample_studio())));

        assert!(response.is_embed());
        let value = serde_json::to_value(response.unwrap_embed()).unwrap();
        assert_eq!(value["url"], "https://anilist.co/studio/2");
    }

    #[test]
    fn not_found_returns_clear_content() {
        let response = handle_studio(Ok(None));

        assert!(response.is_content());
        assert_eq!(response.unwrap_content(), NOT_FOUND_STUDIO);
    }

    #[test]
    fn fetch_error_returns_retryable_content() {
        let response = handle_studio(Err(StudioFetchError::InvalidResponse(
            "bad JSON".to_string(),
        )));

        assert!(response.is_content());
        assert_eq!(response.unwrap_content(), STUDIO_LOOKUP_ERROR);
    }
}
