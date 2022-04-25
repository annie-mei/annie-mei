use super::fetchers::{fetch_by_id, fetch_by_name, queries::*};
use crate::models::anime::Anime;
use crate::models::{
    anime_id_response::FetchResponse as AnimeIdResponse,
    media_list_response::FetchResponse as MediaListResponse,
};
use tokio::task;
use tracing::info;

enum Argument {
    Id(u32),
    Search(String),
}

// TODO: Different fetchers for AniList and MAL
// MAL has song data
impl Argument {
    fn fetch_and_unwrap(&self) -> Anime {
        match self {
            Self::Id(value) => {
                let fetched_data = fetch_by_id(FETCH_ANIME_BY_ID.to_string(), *value);
                let fetch_response: AnimeIdResponse = serde_json::from_str(&fetched_data).unwrap();
                info!("Deserialized response: {:#?}", fetch_response);
                let result: Anime = fetch_response.data.unwrap().media.unwrap();
                result
            }
            Self::Search(value) => {
                let fetched_data = fetch_by_name(FETCH_ANIME.to_string(), value.to_string());
                let fetch_response: MediaListResponse =
                    serde_json::from_str(&fetched_data).unwrap();
                info!("Deserialized response: {:#?}", fetch_response);
                let result: Anime = fetch_response.fuzzy_match(value.to_string());
                info!("Fuzzy Response: {:#?}", result);
                result
            }
        }
    }
}

fn return_argument(arg: &str) -> Argument {
    match arg.parse::<u32>() {
        Ok(id) => Argument::Id(id),
        Err(_e) => Argument::Search(arg.to_string()),
    }
}

// impl serde::Serialize for Argument {
//     fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
//     where
//         S: serde::Serializer,
//     {
//         let mut request = serializer.serialize_struct("Argument", 2)?;
//         request.serialize_field("query", FETCH_ANIME)?;
//         let mut variables = serializer.serialize_struct("variables", 1)?;
//         match self {
//             Self::Id(id) => {
//                 variables.serialize_field("id", id)?;
//             }
//             Self::Search(search) => {
//                 variables.serialize_field("search", search)?;
//             }
//         }
//         request.serialize_field("variables", &variables.end()?)?;
//         request.end()
//     }
// }

#[tokio::main]
pub async fn fetcher(mut args: serenity::framework::standard::Args) -> Anime {
    // Skips over the first arg because this is the command name
    args.single::<String>().unwrap();
    let args = args.remains().unwrap();
    info!("Found Args: {}", args);

    let argument = return_argument(args);
    let result = task::spawn_blocking(move || argument.fetch_and_unwrap())
        .await
        .expect("Fetching Thread Panicked");

    result
}
