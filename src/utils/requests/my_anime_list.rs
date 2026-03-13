use std::env;

use reqwest::blocking::Client;
use tracing::{info, instrument};

use crate::utils::statics::MAL_CLIENT_ID;

const MY_ANIME_LIST_BASE: &str = "https://api.myanimelist.net/v2";
const FIELDS_TO_FETCH: [&str; 3] = ["id", "opening_themes", "ending_themes"];

fn build_mal_url(mal_id: u32) -> String {
    let mal_url = format!(
        "{MY_ANIME_LIST_BASE}/anime/{mal_id}?fields={}",
        FIELDS_TO_FETCH.join(",")
    );

    info!("Sent MAL Request to URL: {mal_url:#?}");
    mal_url
}

#[instrument(name = "http.mal.send_request", skip_all, fields(mal_id = mal_id))]
pub fn send_request(mal_id: u32) -> Result<String, String> {
    let mal_client_id = env::var(MAL_CLIENT_ID)
        .map_err(|_| "MAL_CLIENT_ID is not configured in the environment".to_string())?;
    let client = Client::new();
    let response = client
        .get(build_mal_url(mal_id))
        .header("X-MAL-CLIENT-ID", mal_client_id)
        .send()
        .map_err(|error| format!("Failed to call MyAnimeList API: {error}"))?;

    response
        .text()
        .map_err(|error| format!("Failed to read MyAnimeList response body: {error}"))
}
