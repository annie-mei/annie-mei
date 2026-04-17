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
    all::{CommandInteraction, CreateCommandOption, CreateEmbed, EditInteractionResponse},
    builder::CreateCommand,
    client::Context,
    model::application::CommandOptionType,
};

use tokio::task;
use tracing::{error, info, instrument};

pub fn register() -> CreateCommand {
    CreateCommand::new("songs")
        .description("Fetches the songs of an anime")
        .add_option(
            CreateCommandOption::new(
                CommandOptionType::String,
                "search",
                "Anilist ID or Search term",
            )
            .required(true),
        )
}

#[instrument(name = "command.songs.run", skip(ctx, interaction))]
pub async fn run(ctx: &Context, interaction: &mut CommandInteraction) {
    let _ = interaction.defer(&ctx.http).await;

    let user = &interaction.user;
    let arg = interaction.data.options[0].value.clone();
    let arg_str = format!("{:?}", arg);

    configure_sentry_scope("Songs", user.id.get(), Some(json!(arg_str)));

    info!("Got command 'songs' with args: {arg:#?}");

    if let serenity::all::CommandDataOptionValue::String(ref search_term) = arg
        && let Err(err) = validate_search_term(search_term)
    {
        let builder = EditInteractionResponse::new().content(format!(
            "Invalid search input: {err}. Please check your input and try again."
        ));
        let _ = interaction.edit_response(&ctx.http, builder).await;
        return;
    }

    let response = SongFetcher(arg).await;

    let _songs_response = match response {
        SongFetchResult::Found(mal_response) => {
            // Pure parsing — no I/O, no spawn_blocking needed
            let openings = mal_response.parse_openings();
            let endings = mal_response.parse_endings();

            // Narrow spawn_blocking: only the sync Spotify + Redis I/O
            let (openings, endings) =
                match task::spawn_blocking(move || enrich_song_sections(openings, endings)).await {
                    Ok(result) => result,
                    Err(err) => {
                        error!(error = %err, "spawn_blocking panicked during Spotify enrichment");
                        let builder = EditInteractionResponse::new().content(
                        "An internal error occurred while fetching songs. Please try again later.",
                    );
                        let _ = interaction.edit_response(&ctx.http, builder).await;
                        return;
                    }
                };

            // Pure formatting — no I/O, no spawn_blocking needed
            let builder = EditInteractionResponse::new().embed(
                CreateEmbed::new()
                    .title(mal_response.transform_title())
                    .field(
                        "Openings",
                        MalResponse::format_parsed_songs(&openings),
                        false,
                    )
                    .field("Endings", MalResponse::format_parsed_songs(&endings), false)
                    .thumbnail(mal_response.transform_thumbnail())
                    .field("\u{200b}", mal_response.transform_mal_link(), false),
            );
            interaction.edit_response(&ctx.http, builder).await
        }
        SongFetchResult::AnimeNotFound => {
            let builder = EditInteractionResponse::new().content(NOT_FOUND_ANIME);
            interaction.edit_response(&ctx.http, builder).await
        }
        SongFetchResult::AnimeNotFoundOnMal => {
            let builder = EditInteractionResponse::new()
                .content("Anime not found on MAL. Song data is only available for anime listed on MyAnimeList.");
            interaction.edit_response(&ctx.http, builder).await
        }
        SongFetchResult::FetchError => {
            let builder = EditInteractionResponse::new()
                .content("An error occurred while fetching song data. Please try again later.");
            interaction.edit_response(&ctx.http, builder).await
        }
    };
}

#[instrument(name = "songs.enrich_spotify_section", skip(openings, endings), fields(openings_len = openings.len(), endings_len = endings.len()))]
fn enrich_song_sections(
    mut openings: Vec<ParsedSong>,
    mut endings: Vec<ParsedSong>,
) -> (Vec<ParsedSong>, Vec<ParsedSong>) {
    enrich_songs_with_spotify(&mut openings);
    enrich_songs_with_spotify(&mut endings);
    (openings, endings)
}

#[cfg(test)]
mod tests {
    use super::*;

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
