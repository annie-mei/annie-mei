use reqwest::Client;
use serde_json::json;
use tracing::info;

const QUERY: &str = "
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
    let client = Client::new();
    let json = json!({"query": QUERY, "variables": {"id":id}});
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
    info!("Fetched By ID: {:#?}", result);

    result
}
