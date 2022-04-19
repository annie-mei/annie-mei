use super::{
    fetchers::{fetch_by_id, fetch_by_name},
    model::Anime,
};

use tokio::task;

enum Argument {
    Id(u32),
    Search(String),
}

// TODO: Different fetchers for AniList and MAL
// MAL has song data
impl Argument {
    fn fetch_and_unwrap(&self) -> Anime {
        let fetched_data = match self {
            Self::Id(value) => fetch_by_id(*value),
            Self::Search(value) => fetch_by_name(value.to_string()),
        };
        let serialized_content: serde_json::Value = serde_json::from_str(&fetched_data).unwrap();
        let serialized_result = serialized_content
            .get("data")
            .and_then(|value| value.get("Media"))
            .unwrap();
        let result: Anime = serde_json::from_str(&serialized_result.to_string()).unwrap();
        result
    }
}

fn return_argument(arg: &str) -> Argument {
    let result = match arg.parse::<u32>() {
        Ok(id) => Argument::Id(id),
        Err(_e) => Argument::Search(arg.to_string()),
    };
    result
}

#[tokio::main]
pub async fn fetcher(mut args: serenity::framework::standard::Args) -> Anime {
    args.single::<String>().unwrap();
    let args = args.remains().unwrap();

    let argument = return_argument(args);
    let result = task::spawn_blocking(move || argument.fetch_and_unwrap())
        .await
        .expect("Fetching Thread Panicked");

    result
}
