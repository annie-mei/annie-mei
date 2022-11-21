use rspotify::{
    model::{Country, Market, SearchResult, SearchType},
    prelude::*,
    ClientCredsSpotify, Credentials,
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

pub fn get_song_url(romaji_name: String, kana_name: String, artist_name: String) -> Option<String> {
    let mut spotify = get_spotify_client();
    spotify.request_token().unwrap();
    let search = spotify.search(
        format!("track:{} artist:{}", romaji_name, artist_name).as_str(),
        &SearchType::Track,
        Some(&Market::Country(Country::UnitedStates)),
        None,
        Some(5),
        None,
    );

    // TODO: Fallback on Kana Name

    match search {
        Ok(search_result) => {
            info!("Searched track: {search_result:#?}");
            match search_result {
                SearchResult::Tracks(page) => {
                    // Gets URL for top result
                    // TODO: Improve this using fuzzy matching
                    return Some(page.items[0].external_urls["spotify"].to_owned());
                }
                _ => info!("Something else"),
            }
        }
        // TODO: Handle Error Case
        Err(err) => info!("Could not find track: {err:#?}"),
    }

    // let tracks = search.tracks();
    // info!("{:#?}", search);
    // TODO: Return URL and use it in the response
    None
}
