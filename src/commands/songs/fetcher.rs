use crate::{
    commands::traits::{AniListSource, MediaDataSource},
    models::{mal_response::MalResponse, transformers::Transformers},
    utils::requests::my_anime_list,
};

use tracing::{info, instrument, warn};

#[derive(Debug)]
pub enum SongFetchError {
    AnimeNotFound,
    MissingMyAnimeListId,
    UpstreamUnavailable,
    MalformedUpstreamResponse,
}

#[instrument(name = "command.songs.fetcher", skip(search_term), fields(search_len = search_term.len()))]
pub fn fetcher(search_term: &str) -> Result<MalResponse, SongFetchError> {
    let anime = AniListSource
        .fetch_anime(search_term)
        .ok_or(SongFetchError::AnimeNotFound)?;
    let mal_id = anime
        .get_mal_id()
        .ok_or(SongFetchError::MissingMyAnimeListId)?;

    let mal_fetcher_response = my_anime_list::send_request(mal_id).map_err(|error| {
        warn!(error = %error, mal_id, "Failed to fetch MAL response");
        SongFetchError::UpstreamUnavailable
    })?;

    let mal_response: MalResponse =
        serde_json::from_str(&mal_fetcher_response).map_err(|error| {
            warn!(error = %error, mal_id, "Failed to deserialize MAL response");
            SongFetchError::MalformedUpstreamResponse
        })?;

    info!(mal_id, "Fetched MAL response");
    Ok(mal_response)
}
