use crate::{
    models::{
        anilist_anime::Anime, mal_response::MalResponse, media_type::MediaType as Type,
        transformers::Transformers,
    },
    utils::{requests::my_anime_list, response_fetcher::fetcher as anime_fetcher},
};

use serenity::all::CommandDataOptionValue;
use tracing::{error, info, instrument};

pub enum SongFetchResult {
    Found(MalResponse),
    AnimeNotFound,
    AnimeNotFoundOnMal,
    FetchError,
}

#[instrument(name = "command.songs.fetcher", skip(args))]
pub async fn fetcher(args: CommandDataOptionValue) -> SongFetchResult {
    let anime_response: Option<Anime> = anime_fetcher(Type::Anime, args).await;
    let Some(anime) = anime_response else {
        return SongFetchResult::AnimeNotFound;
    };

    let Some(mal_id) = anime.get_mal_id() else {
        info!("Anime found on AniList but has no MAL ID");
        return SongFetchResult::AnimeNotFoundOnMal;
    };

    let mal_fetcher_response = match my_anime_list::send_request(mal_id).await {
        Ok(response) => response,
        Err(err) => {
            error!(error = %err, mal_id = mal_id, "Failed to fetch MAL data for anime");
            return SongFetchResult::FetchError;
        }
    };

    let mal_response: MalResponse = match serde_json::from_str(&mal_fetcher_response) {
        Ok(response) => response,
        Err(err) => {
            error!(error = %err, mal_id = mal_id, "Failed to deserialize MAL response");
            return SongFetchResult::FetchError;
        }
    };

    info!("Mal Response: {:#?}", mal_response);
    SongFetchResult::Found(mal_response)
}
