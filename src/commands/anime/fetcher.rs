use reqwest::Client;
use serde_json::json;

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
pub async fn fetcher() -> serde_json::Value {
    let client = Client::new();
    let json = json!({"query": QUERY, "variables": {"id":1}});

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
    println!("{:#?}", result);

    result
}
