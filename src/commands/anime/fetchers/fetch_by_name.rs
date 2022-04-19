use reqwest::Client;
use serde_json::json;
use tracing::info;

const QUERY: &str = "
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
    let client = Client::new();
    let json = json!({"query": QUERY, "variables": {"search":name}});
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

    // TODO: unwrap this using structs
    // https://ectobit.com/blog/parsing-json-in-rust/
    let result: serde_json::Value = serde_json::from_str(&response.unwrap()).unwrap();
    info!("Fetched By Name: {:#?}", result);

    result
}
