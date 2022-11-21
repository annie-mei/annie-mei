use rspotify::{
    model::{Country, Market, SearchResult, SearchType},
    prelude::*,
    ClientCredsSpotify, ClientError, Credentials,
};

use std::env;
use tracing::info;

fn get_spotify_client() -> ClientCredsSpotify {
    let client_id =
        env::var("SPOTIFY_CLIENT_ID").expect("Expected a spotify client id in the environment");
    let client_secret = env::var("SPOTIFY_CLIENT_SECRET")
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
    let romaji_search = send_search_request(&romaji_name, &artist_name);
    match romaji_search {
        Ok(search_result) => {
            info!("Searched track: {search_result:#?}");
            if let Some(url) = get_url_from_search_result(search_result) {
                return Some(url);
            } else if let Some(kana_name) = kana_name {
                let kana_search = send_search_request(&kana_name, &artist_name);
                match kana_search {
                    Ok(search_result) => {
                        info!("Searched track using Track: {kana_name:#?} Artist: {artist_name:#?}: {search_result:#?}");
                        match get_url_from_search_result(search_result) {
                            Some(url) => return Some(url),
                            None => return None,
                        }
                    }
                    Err(e) => {
                        info!("Error searching track: {e:#?}");
                        return None;
                    }
                }
            } else {
                return None;
            }
        }
        Err(err) => info!("Could not find track: {err:#?}"),
    }
    None
}

fn send_search_request(
    song_name: &String,
    artist_name: &String,
) -> Result<SearchResult, ClientError> {
    let mut spotify = get_spotify_client();
    spotify.request_token().unwrap();
    spotify.search(
        format!("track:{} artist:{}", song_name, artist_name).as_str(),
        &SearchType::Track,
        Some(&Market::Country(Country::UnitedStates)),
        None,
        Some(5),
        None,
    )
}

fn get_url_from_search_result(search_result: SearchResult) -> Option<String> {
    match search_result {
        SearchResult::Tracks(page) => {
            // Gets URL for top result
            // TODO: Improve this using fuzzy matching instead of just taking the first result
            if !page.items.is_empty() {
                let track = &page.items[0];
                info!("Found track: {track:#?}");
                return Some(track.external_urls["spotify"].to_owned());
            }
            None
        }
        _ => {
            info!("Something else");
            None
        }
    }
}
