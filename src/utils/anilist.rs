use reqwest::blocking::Client;
use serde_json::Value;

pub fn send_request(json: Value) -> String {
    let client = Client::new();
    let response = client
        .post("https://graphql.anilist.co/")
        .header("Content-Type", "application/json")
        .header("Accept", "application/json")
        .body(json.to_string())
        .send()
        .unwrap()
        .text();

    let result = &response.unwrap();

    result.to_string()
}
