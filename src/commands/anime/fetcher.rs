use super::fetchers::{fetch_by_id, fetch_by_name, queries::*};
use crate::models::anime::Anime;
use crate::models::fetch::FetchResponse;
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
            Self::Id(value) => fetch_by_id(FETCH_BY_ID_QUERY.to_string(), *value),
            Self::Search(value) => {
                fetch_by_name(FETCH_BY_SEARCH_QUERY.to_string(), value.to_string())
            }
        };

        // TODO: Levenshtein this Shit
        let fetch_response: FetchResponse = serde_json::from_str(&fetched_data).unwrap();
        let result: Anime = fetch_response.data.unwrap().media.unwrap();
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
