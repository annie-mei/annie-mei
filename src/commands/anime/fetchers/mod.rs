use reqwest::Client;
use serde_json::{json, Value};
use tracing::info;

pub mod queries;

async fn send_request(json: Value) -> String {
    let client = Client::new();
    let response = client
        .post("https://graphql.anilist.co/")
        .header("Content-Type", "application/json")
        .header("Accept", "application/json")
        .body(json.to_string())
        .send()
        .await
        .unwrap()
        .text()
        .await;

    let result = &response.unwrap();

    result.to_string()
}

#[tokio::main]
pub async fn fetch_by_id(query: String, id: u32) -> String {
    let json = json!({"query": query, "variables": {"id":id}});
    let result: String = send_request(json).await;

    info!("Fetched By ID: {:#?}", result);

    result
}

#[tokio::main]
pub async fn fetch_by_name(query: String, name: String) -> String {
    let json = json!({"query": query, "variables": {"search":name}});
    let result: String = send_request(json).await;

    info!("Fetched By Name: {:#?}", result);

    result
}
