use crate::{
    commands::{
        anime::queries::{FETCH_ANIME, FETCH_ANIME_BY_ID},
        manga::queries::{FETCH_MANGA, FETCH_MANGA_BY_ID},
    },
    models::{
        id_response::FetchResponse as IdResponse, media_response::FetchResponse as MediaResponse,
        media_type::MediaType as Type, transformers::Transformers,
    },
    utils::{
        fetch_by_arguments::{fetch_by_id, fetch_by_name},
        redis::{check_cache, try_to_cache_response},
    },
};

use tokio::task;
use tracing::{debug, error, info, instrument};

pub struct AnimeConfig {
    argument: Argument,
    id_query: String,
    search_query: String,
}

pub struct MangaConfig {
    argument: Argument,
    id_query: String,
    search_query: String,
}

pub enum Argument {
    Id(u32),
    Search(String),
}

pub trait Response {
    fn new(argument: Argument) -> Self;
    fn get_argument(&self) -> &Argument;
    fn get_id_query(&self) -> String;
    fn get_search_query(&self) -> String;
}

#[instrument(
    name = "anilist.fetch_from_network_and_cache",
    skip(search_query),
    fields(cache_key = %cache_key, lookup_len = lookup_value.len())
)]
async fn fetch_from_network_and_cache(
    search_query: String,
    lookup_value: String,
    cache_key: String,
) -> Option<String> {
    let response = match fetch_by_name(search_query, lookup_value).await {
        Ok(data) => data,
        Err(err) => {
            error!(error = %err, "Failed to fetch AniList data by name");
            return None;
        }
    };

    let cache_key_for_write = cache_key.clone();
    let response_to_cache = response.clone();
    if let Err(err) = task::spawn_blocking(move || {
        try_to_cache_response(&cache_key_for_write, &response_to_cache)
    })
    .await
    {
        error!(error = %err, cache_key = %cache_key, "Failed to cache AniList response");
    }

    Some(response)
}

#[instrument(name = "anilist.fetch", skip(response_config), fields(media_type = ?media_type))]
pub async fn fetch<
    T: serde::de::DeserializeOwned + Transformers + std::fmt::Debug + std::clone::Clone,
>(
    response_config: &impl Response,
    media_type: Type,
) -> Option<T> {
    match response_config.get_argument() {
        Argument::Id(value) => {
            let fetched_data = match fetch_by_id(response_config.get_id_query(), *value).await {
                Ok(data) => data,
                Err(err) => {
                    error!(error = %err, id = *value, "Failed to fetch AniList data by id");
                    return None;
                }
            };
            let fetch_response: IdResponse<T> = match serde_json::from_str(&fetched_data) {
                Ok(response) => response,
                Err(err) => {
                    error!(error = %err, "Failed to deserialize AniList id response");
                    return None;
                }
            };
            debug!("Deserialized response: {:#?}", fetch_response);
            fetch_response.data.and_then(|data| data.media)
        }
        Argument::Search(value) => {
            let cache_key = format!("{}:{value}", media_type.as_ref());
            let cache_key_for_lookup = cache_key.clone();
            let search_query = response_config.get_search_query();
            let lookup_value = value.to_string();

            let fetched_data =
                match task::spawn_blocking(move || check_cache(&cache_key_for_lookup)).await {
                    Ok(Ok(cached_value)) => {
                        info!("Cache hit for {:#?}", cache_key);
                        cached_value
                    }
                    Ok(Err(err)) => {
                        info!("Cache miss for {:#?} with error {:#?}", cache_key, err);
                        fetch_from_network_and_cache(
                            search_query.clone(),
                            lookup_value.clone(),
                            cache_key.clone(),
                        )
                        .await?
                    }
                    Err(err) => {
                        error!(error = %err, "Failed to read AniList cache");
                        fetch_from_network_and_cache(search_query, lookup_value, cache_key.clone())
                            .await?
                    }
                };
            let fetch_response: MediaResponse<T> = match serde_json::from_str(&fetched_data) {
                Ok(response) => response,
                Err(err) => {
                    error!(error = %err, "Failed to deserialize AniList search response");
                    return None;
                }
            };
            debug!("Deserialized response: {:#?}", fetch_response);
            let result = fetch_response.fuzzy_match(value, media_type);
            debug!("Fuzzy Response: {:#?}", result);
            result
        }
    }
}

impl Response for AnimeConfig {
    fn new(argument: Argument) -> AnimeConfig {
        AnimeConfig {
            argument,
            id_query: FETCH_ANIME_BY_ID.to_string(),
            search_query: FETCH_ANIME.to_string(),
        }
    }

    fn get_argument(&self) -> &Argument {
        &self.argument
    }

    fn get_id_query(&self) -> String {
        self.id_query.to_owned()
    }

    fn get_search_query(&self) -> String {
        self.search_query.to_owned()
    }
}

impl Response for MangaConfig {
    fn new(argument: Argument) -> MangaConfig {
        MangaConfig {
            argument,
            id_query: FETCH_MANGA_BY_ID.to_string(),
            search_query: FETCH_MANGA.to_string(),
        }
    }

    fn get_argument(&self) -> &Argument {
        &self.argument
    }

    fn get_id_query(&self) -> String {
        self.id_query.to_owned()
    }

    fn get_search_query(&self) -> String {
        self.search_query.to_owned()
    }
}
