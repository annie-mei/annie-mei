use crate::{
    commands::{
        response::CommandResponse,
        traits::{AniListSource, CharacterDataSource},
    },
    models::anilist_character::Character,
    utils::{
        channel::is_nsfw_channel,
        privacy::configure_sentry_scope,
        statics::{NOT_FOUND_CHARACTER, NSFW_NOT_ALLOWED},
    },
};

use serde_json::json;
use serenity::{
    all::{CommandInteraction, CreateCommandOption, EditInteractionResponse},
    builder::CreateCommand,
    client::Context,
    model::application::CommandOptionType,
};
use tracing::{info, instrument};

pub fn register() -> CreateCommand {
    CreateCommand::new("character")
        .description("Fetches the details for an AniList character")
        .add_option(
            CreateCommandOption::new(
                CommandOptionType::String,
                "search",
                "AniList character ID or search term",
            )
            .required(true),
        )
}

pub fn handle_character(character: Option<Character>, allow_adult_media: bool) -> CommandResponse {
    match character {
        None => CommandResponse::Content(NOT_FOUND_CHARACTER.to_string()),
        Some(character_response) => CommandResponse::Embed(Box::new(
            character_response.transform_response_embed(allow_adult_media),
        )),
    }
}

#[instrument(name = "command.character.run", skip(ctx, interaction))]
pub async fn run(ctx: &Context, interaction: &mut CommandInteraction) {
    let _ = interaction.defer(&ctx.http).await;

    let user = &interaction.user;

    let Some(serenity::all::CommandDataOptionValue::String(search_term)) =
        interaction.data.options.first().map(|opt| &opt.value)
    else {
        let builder = EditInteractionResponse::new()
            .content("Missing or invalid `search` option — please provide a character name or ID.");
        let _ = interaction.edit_response(&ctx.http, builder).await;
        return;
    };
    let search_term = search_term.clone();

    let arg_str = format!("{:?}", search_term);
    configure_sentry_scope("Character", user.id.get(), Some(json!(arg_str)));

    info!("Got command 'character' with search_term: {search_term}");

    let character_result = AniListSource.fetch_character(&search_term).await;
    let allow_adult_media = if character_result
        .as_ref()
        .is_some_and(Character::has_adult_media)
    {
        is_nsfw_channel(ctx, interaction.channel_id).await
    } else {
        false
    };

    if let Some(ref character) = character_result
        && character.media_is_all_adult()
        && !allow_adult_media
    {
        let builder = EditInteractionResponse::new().content(NSFW_NOT_ALLOWED);
        let _ = interaction.edit_response(&ctx.http, builder).await;
        return;
    }

    let response = handle_character(character_result, allow_adult_media);

    let _result = match response {
        CommandResponse::Content(text) => {
            let builder = EditInteractionResponse::new().content(text);
            interaction.edit_response(&ctx.http, builder).await
        }
        CommandResponse::Embed(embed) => {
            let builder = EditInteractionResponse::new().embed(*embed);
            interaction.edit_response(&ctx.http, builder).await
        }
        CommandResponse::Message(text) => {
            let builder = EditInteractionResponse::new().content(text);
            interaction.edit_response(&ctx.http, builder).await
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_character() -> Character {
        serde_json::from_value(serde_json::json!({
            "id": 40,
            "name": {
                "full": "Lelouch Lamperouge",
                "native": "ルルーシュ・ランペルージ",
                "alternative": [],
                "userPreferred": "Lelouch Lamperouge"
            },
            "image": {
                "large": "https://example.com/large.jpg",
                "medium": null
            },
            "description": "<p>A former prince.</p>",
            "gender": "Male",
            "dateOfBirth": { "year": null, "month": 12, "day": 5 },
            "age": "17",
            "bloodType": "A",
            "favourites": 1000,
            "siteUrl": "https://anilist.co/character/40",
            "media": { "nodes": [] }
        }))
        .expect("sample character JSON should deserialize")
    }

    #[test]
    fn character_not_found_returns_content_with_message() {
        let response = handle_character(None, false);

        assert!(response.is_content(), "expected Content variant");
        assert_eq!(response.unwrap_content(), NOT_FOUND_CHARACTER);
    }

    #[test]
    fn character_success_returns_embed() {
        let response = handle_character(Some(sample_character()), false);

        assert!(
            response.is_embed(),
            "expected Embed variant for a successful lookup"
        );
        let _embed = response.unwrap_embed();
    }
}
