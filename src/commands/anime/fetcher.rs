use reqwest::Client;
use serde_json::json;

enum Argument {
    Id(u32),
    Search(String),
}

impl Argument {
    fn fetch(&self) {
        match self {
            Self::Id(value) => println!("FETCH FROM ID: {:#?}", value),
            Self::Search(value) => println!("FETCH FROM SEARCH {:#?}", value),
        }
    }
}

fn return_argument(arg: &str) -> Argument {
    let result = match arg.parse::<u32>() {
        Ok(id) => Argument::Id(id),
        Err(_e) => Argument::Search(arg.to_string()),
    };
    result
}

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
pub async fn fetcher(mut args: serenity::framework::standard::Args) -> serde_json::Value {
    args.single::<String>().unwrap();
    let args = args.remains().unwrap();

    let argument = return_argument(args);

    println!("{:#?}", argument.fetch());

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
