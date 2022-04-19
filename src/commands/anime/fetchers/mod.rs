use reqwest::Client;
use serde_json::{json, Value};
use tracing::info;

async fn send_request(json: Value) -> serde_json::Value {
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

    let result: serde_json::Value = serde_json::from_str(&response.unwrap()).unwrap();
    result
}

const FETCH_BY_ID_QUERY: &str = "
query ($id: Int) {
  Media (id: $id, type: ANIME) {
    id
    title {
      romaji
      english
      native
    }
  }
}
";

#[tokio::main]
pub async fn fetch_by_id(id: u32) -> serde_json::Value {
    // TODO: unwrap this using structs
    // https://ectobit.com/blog/parsing-json-in-rust/
    let json = json!({"query": FETCH_BY_ID_QUERY, "variables": {"id":id}});
    let result: serde_json::Value = send_request(json).await;

    info!("Fetched By ID: {:#?}", result);

    result
}

const FETCH_BY_SEARCH_QUERY: &str = "
query ($search: String) {
  Media (search: $search, type: ANIME) {
    id
    title {
      romaji
      english
      native
    }
  }
}
";

#[tokio::main]
pub async fn fetch_by_name(name: String) -> serde_json::Value {
    // TODO: unwrap this using structs
    // https://ectobit.com/blog/parsing-json-in-rust/
    let json = json!({"query": FETCH_BY_SEARCH_QUERY, "variables": {"search":name}});
    let result: serde_json::Value = send_request(json).await;

    info!("Fetched By Name: {:#?}", result);

    result
}
