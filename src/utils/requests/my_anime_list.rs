use std::env;

use reqwest::blocking::Client;
use tracing::info;

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

pub fn send_request(mal_id: u32) -> String {
    let mal_client_id =
        env::var(MAL_CLIENT_ID).expect("Expected a MAL Client ID in the environment");
    let client = Client::new();
    let response = client
        .get(build_mal_url(mal_id))
        .header("X-MAL-CLIENT-ID", mal_client_id)
        .send()
        .unwrap()
        .text();

    let result = &response.unwrap();

    result.to_string()
}
