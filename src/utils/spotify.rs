use crate::utils::{
    redis::{check_cache, try_to_cache_response},
    statics::{SPOTIFY_CLIENT_ID, SPOTIFY_CLIENT_SECRET},
};

use rspotify::{
    ClientCredsSpotify, ClientError, Credentials,
    model::{Country, Market, SearchResult, SearchType},
    prelude::*,
};

use std::env;
use tracing::info;

fn get_spotify_client() -> ClientCredsSpotify {
    let client_id =
        env::var(SPOTIFY_CLIENT_ID).expect("Expected a spotify client id in the environment");
    let client_secret = env::var(SPOTIFY_CLIENT_SECRET)
        .expect("Expected a spotify client secret in the environment");
    let credentials = Credentials {
        id: client_id,
        secret: Some(client_secret),
    };
    let spotify = ClientCredsSpotify::new(credentials);
    info!("Spotify client established");
    spotify
}

pub fn get_song_url(
    romaji_name: String,
    kana_name: Option<String>,
    artist_name: String,
) -> Option<String> {
    // If cached response if found, return it
    let cache_key = format!("{romaji_name}:{kana_name:#?}:{artist_name}");
    match check_cache(&cache_key) {
        Ok(value) => {
            info!("Cache hit for {:#?}", cache_key);
            return match value.as_str() {
                "None" => None,
                _ => Some(value),
            };
        }
        Err(e) => {
            info!("Cache miss for {:#?} with error {:#?}", cache_key, e);
        }
    };

    let romaji_search = send_search_request(&romaji_name, &artist_name);
    match romaji_search {
        Ok(search_result) => {
            info!("Searched track: {search_result:#?}");
            if let Some(url) = get_url_from_search_result(search_result) {
                try_to_cache_response(&cache_key, &url);
                return Some(url);
            } else if let Some(kana_name) = kana_name {
                let kana_search = send_search_request(&kana_name, &artist_name);
                match kana_search {
                    Ok(search_result) => {
                        info!(
                            "Searched track using Track: {kana_name:#?} Artist: {artist_name:#?}: {search_result:#?}"
                        );
                        match get_url_from_search_result(search_result) {
                            Some(url) => {
                                try_to_cache_response(&cache_key, &url);
                                return Some(url);
                            }
                            None => {
                                try_to_cache_response(&cache_key, "None");
                                return None;
                            }
                        }
                    }
                    Err(e) => {
                        info!("Error searching track: {e:#?}");
                    }
                }
            } else {
                try_to_cache_response(&cache_key, "None");
                return None;
            }
        }
        Err(err) => info!("Could not find track: {err:#?}"),
    }
    try_to_cache_response(&cache_key, "None");
    None
}

fn send_search_request(
    song_name: &String,
    artist_name: &String,
) -> Result<SearchResult, ClientError> {
    let spotify = get_spotify_client();
    spotify.request_token().unwrap();
    spotify.search(
        format!("track:{song_name} artist:{artist_name}").as_str(),
        SearchType::Track,
        Some(Market::Country(Country::UnitedStates)),
        None,
        Some(5),
        None,
    )
}

fn get_url_from_search_result(search_result: SearchResult) -> Option<String> {
    if let SearchResult::Tracks(page) = search_result {
        // Gets URL for top result
        if !page.items.is_empty() {
            let track = &page.items[0];
            info!("Found track: {track:#?}");
            return Some(track.external_urls["spotify"].to_owned());
        }
        None
    } else {
        info!("Something else");
        None
    }
}
