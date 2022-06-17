use crate::commands::anime::queries::{FETCH_ANIME, FETCH_ANIME_BY_ID};
use crate::models::anilist_anime::Anime;
use crate::models::{
    anime_id_response::FetchResponse as AnimeIdResponse,
    media_list_response::FetchResponse as MediaListResponse,
};
use crate::utils::fetchers::fetch_by_arguments::{fetch_by_id, fetch_by_name};
use tracing::info;

pub struct AnimeResponse {
    argument: Argument,
    id_query: String,
    search_query: String,
}

pub struct MangaResponse {
    argument: Argument,
    id_query: String,
    search_query: String,
}

pub enum Argument {
    Id(u32),
    Search(String),
}

pub trait Response {
    fn new(argument: Argument) -> Self;
    fn fetch(&self) -> Option<Anime>;
}

impl Response for AnimeResponse {
    fn new(argument: Argument) -> AnimeResponse {
        AnimeResponse {
            argument,
            id_query: FETCH_ANIME_BY_ID.to_string(),
            search_query: FETCH_ANIME.to_string(),
        }
    }

    // TODO: Move this to default implementation to make it more reusable
    fn fetch(&self) -> Option<Anime> {
        match &self.argument {
            Argument::Id(value) => {
                let fetched_data = fetch_by_id(self.id_query.clone(), *value);
                let fetch_response: AnimeIdResponse = serde_json::from_str(&fetched_data).unwrap();
                info!("Deserialized response: {:#?}", fetch_response);
                let result: Anime = fetch_response.data.unwrap().media.unwrap();
                Some(result)
            }
            Argument::Search(value) => {
                let fetched_data = fetch_by_name(self.search_query.clone(), value.to_string());
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

// TODO: Make return type enum(Anime, Manga) ==> MediaType?? ==> Is this idiomatic?

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
