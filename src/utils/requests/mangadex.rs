use reqwest::blocking::Client;
use tracing::info;

const MANGADEX_BASE: &str = "https://api.mangadex.org";

fn build_mangadex_url(name: String) -> String {
    let mangadex_url = format!("{}/manga?title={}limit=1", MANGADEX_BASE, name,);

    info!("Sent MangaDex Request to URL: {:#?}", mangadex_url);
    mangadex_url
}

pub fn send_request(name: String) -> String {
    let client = Client::new();
    let response = client.get(build_mangadex_url(name)).send().unwrap().text();

    let result = &response.unwrap();

    result.to_string()
}
