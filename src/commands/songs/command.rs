use crate::{
    commands::{
        input_validation::validate_search_term,
        songs::fetcher::{SongFetchResult, fetcher as SongFetcher},
    },
    models::mal_response::{MalResponse, ParsedSong},
    utils::{
        privacy::configure_sentry_scope, spotify::enrich_songs_with_spotify,
        statics::NOT_FOUND_ANIME,
    },
};

use serde_json::json;
use serenity::{
    all::{
        CommandDataOption, CommandDataOptionValue, CommandInteraction, CreateCommandOption,
        CreateEmbed, EditInteractionResponse,
    },
    builder::CreateCommand,
    client::Context,
    model::application::CommandOptionType,
};

use tokio::task;
use tracing::{error, info, instrument};

const SEARCH_OPTION: &str = "search";

pub fn register() -> CreateCommand {
    CreateCommand::new("songs")
        .description("Find anime opening and ending theme songs")
        .add_option(
            CreateCommandOption::new(
                CommandOptionType::String,
                SEARCH_OPTION,
                "AniList ID or anime search term",
            )
            .required(true),
        )
}

#[instrument(name = "command.songs.parse_options", skip(options))]
fn parse_songs_options(options: &[CommandDataOption]) -> Option<String> {
    options
        .iter()
        .find(|option| option.name == SEARCH_OPTION)
        .and_then(|option| match &option.value {
            CommandDataOptionValue::String(search_term) => Some(search_term.clone()),
            _ => None,
        })
}

#[instrument(name = "command.songs.run", skip(ctx, interaction))]
pub async fn run(ctx: &Context, interaction: &mut CommandInteraction) {
    let _ = interaction.defer(&ctx.http).await;

    let user = &interaction.user;

    let Some(search_term) = parse_songs_options(&interaction.data.options) else {
        let builder = EditInteractionResponse::new()
            .content("Tell me what to look up with `search:<anime title or AniList ID>`.");
        let _ = interaction.edit_response(&ctx.http, builder).await;
        return;
    };

    configure_sentry_scope("Songs", user.id.get(), Some(json!(search_term.clone())));

    info!("Got command 'songs' with args: {search_term:#?}");

    if let Err(err) = validate_search_term(&search_term) {
        let builder = EditInteractionResponse::new().content(format!(
            "I couldn't use that search: {err}. Try an anime title or AniList ID."
        ));
        let _ = interaction.edit_response(&ctx.http, builder).await;
        return;
    }

    let response = SongFetcher(CommandDataOptionValue::String(search_term)).await;

    let _songs_response = match response {
        SongFetchResult::Found(mal_response) => {
            // Pure parsing — no I/O, no spawn_blocking needed
            let openings = mal_response.parse_openings();
            let endings = mal_response.parse_endings();

            // Narrow spawn_blocking: only the sync Spotify + Redis I/O
            let (openings, endings) = match task::spawn_blocking(move || {
                enrich_song_sections(openings, endings)
            })
            .await
            {
                Ok(result) => result,
                Err(err) => {
                    error!(error = %err, "spawn_blocking panicked during Spotify enrichment");
                    let builder = EditInteractionResponse::new().content(
                        "I found the anime, but something went wrong while checking theme song links. Please try again later.",
                    );
                    let _ = interaction.edit_response(&ctx.http, builder).await;
                    return;
                }
            };

            // Pure formatting — no I/O, no spawn_blocking needed
            let mut embed = CreateEmbed::new()
                .title(mal_response.transform_title())
                .field(
                    "Opening themes",
                    MalResponse::format_parsed_songs(&openings),
                    false,
                )
                .field(
                    "Ending themes",
                    MalResponse::format_parsed_songs(&endings),
                    false,
                );

            if let Some(thumbnail) = mal_response.transform_thumbnail() {
                embed = embed.thumbnail(thumbnail);
            }

            let builder = EditInteractionResponse::new().embed(embed.field(
                "Source",
                mal_response.transform_mal_link(),
                false,
            ));
            interaction.edit_response(&ctx.http, builder).await
        }
        SongFetchResult::AnimeNotFound => {
            let builder = EditInteractionResponse::new().content(NOT_FOUND_ANIME);
            interaction.edit_response(&ctx.http, builder).await
        }
        SongFetchResult::AnimeNotFoundOnMal => {
            let builder = EditInteractionResponse::new().content(
                "I found that anime on AniList, but couldn't find a MyAnimeList page for its theme songs.",
            );
            interaction.edit_response(&ctx.http, builder).await
        }
        SongFetchResult::FetchError => {
            let builder = EditInteractionResponse::new()
                .content("I couldn't fetch theme song data right now. Please try again later.");
            interaction.edit_response(&ctx.http, builder).await
        }
    };
}

#[instrument(name = "songs.enrich_spotify_section", skip(openings, endings), fields(openings_len = openings.len(), endings_len = endings.len()))]
fn enrich_song_sections(
    mut openings: Vec<ParsedSong>,
    mut endings: Vec<ParsedSong>,
) -> (Vec<ParsedSong>, Vec<ParsedSong>) {
    let opening_count = openings.len();
    openings.append(&mut endings);
    enrich_songs_with_spotify(&mut openings);
    let endings = openings.split_off(opening_count);
    (openings, endings)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn string_option(name: &str, value: &str) -> CommandDataOption {
        serde_json::from_value(serde_json::json!({
            "name": name,
            "type": 3,
            "value": value
        }))
        .expect("option should deserialize")
    }

    fn integer_option(name: &str, value: i64) -> CommandDataOption {
        serde_json::from_value(serde_json::json!({
            "name": name,
            "type": 4,
            "value": value
        }))
        .expect("option should deserialize")
    }

    #[test]
    fn parse_songs_options_extracts_search_term() {
        let options = vec![string_option(SEARCH_OPTION, "Fullmetal Alchemist")];

        assert_eq!(
            parse_songs_options(&options),
            Some("Fullmetal Alchemist".to_string())
        );
    }

    #[test]
    fn parse_songs_options_handles_missing_options() {
        assert_eq!(parse_songs_options(&[]), None);
    }

    #[test]
    fn parse_songs_options_ignores_incorrectly_typed_options() {
        let options = vec![integer_option(SEARCH_OPTION, 5114)];

        assert_eq!(parse_songs_options(&options), None);
    }

    #[test]
    fn parse_songs_options_ignores_unknown_options() {
        let options = vec![string_option("query", "Fullmetal Alchemist")];

        assert_eq!(parse_songs_options(&options), None);
    }

    fn song_without_artist(song_name: &str, display_number: u32) -> ParsedSong {
        ParsedSong {
            display_number,
            song_name: song_name.to_string(),
            romaji_name: song_name.to_string(),
            kana_name: None,
            artist_names: None,
            episode_numbers: None,
            spotify_url: None,
        }
    }

    #[test]
    fn enrich_song_sections_leaves_songs_without_artists_unchanged() {
        let openings = vec![song_without_artist("Opening Song", 1)];
        let endings = vec![song_without_artist("Ending Song", 2)];

        let (openings, endings) = enrich_song_sections(openings, endings);

        assert_eq!(openings.len(), 1);
        assert_eq!(endings.len(), 1);
        assert_eq!(openings[0].song_name, "Opening Song");
        assert_eq!(endings[0].song_name, "Ending Song");
        assert!(openings[0].spotify_url.is_none());
        assert!(endings[0].spotify_url.is_none());
    }
}
