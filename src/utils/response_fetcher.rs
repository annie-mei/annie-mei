use crate::commands::anime::queries::{FETCH_ANIME_BY_ID, FETCH_ANIME};
use crate::models::anilist_anime::Anime;
use crate::models::{
    anime_id_response::FetchResponse as AnimeIdResponse,
    media_list_response::FetchResponse as MediaListResponse,
};
use crate::utils::fetchers::fetch_by_arguments::{fetch_by_name, fetch_by_id};
use tracing::info;

enum Argument {
    Id(u32),
    Search(String),
}


// TODO: Make return type enum(Anime, Manga, Songs)?? ==> Is this idiomatic?
// TODO: TRAITS CAN BE OVERWRITTEN
impl Argument {
    // TODO: Make this return a Result?? and add Proper result error handling -> SNAFU
    fn fetch_and_unwrap(&self, _query_type: &str) -> Option<Anime> {
        match self {
            Self::Id(value) => {
                let fetched_data = fetch_by_id(FETCH_ANIME_BY_ID.to_string(), *value);
                let fetch_response: AnimeIdResponse = serde_json::from_str(&fetched_data).unwrap();
                info!("Deserialized response: {:#?}", fetch_response);
                let result: Anime = fetch_response.data.unwrap().media.unwrap();
                Some(result)
            }
            Self::Search(value) => {
                let fetched_data = fetch_by_name(FETCH_ANIME.to_string(), value.to_string());
                let fetch_response: MediaListResponse =
                    serde_json::from_str(&fetched_data).unwrap();
                info!("Deserialized response: {:#?}", fetch_response);
                let result: Option<Anime> = fetch_response.fuzzy_match(value);
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

pub fn fetcher(mut args: serenity::framework::standard::Args) -> Option<Anime> {
    // Skips over the first arg because this is the command name
    info!("Found Args: {:#?}", args);
    // TODO: This should be passed as an Enum
    let query_type= &args.single::<String>().unwrap()[1..];
    info!("Detected query of type: {:#?}", query_type);

    let args = args.remains().unwrap();

    let argument = return_argument(args);
    argument.fetch_and_unwrap(query_type)
}

// TODO: Custom deserializer?
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
