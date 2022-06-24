use crate::commands::{
    anime::queries::{FETCH_ANIME, FETCH_ANIME_BY_ID},
    manga::queries::{FETCH_MANGA, FETCH_MANGA_BY_ID},
};
use crate::models::anilist_anime::Anime;
use crate::models::{
    anime_id_response::FetchResponse as AnimeIdResponse,
    manga_id_response::FetchResponse as MangaIdResponse,
    media_list_response::FetchResponse as MediaListResponse,
    media_type::MediaResponse as ResponseType,
};
use crate::utils::fetchers::fetch_by_arguments::{fetch_by_id, fetch_by_name};
use tracing::info;

pub struct AnimeConfig {
    argument: Argument,
    id_query: String,
    search_query: String,
}

pub struct MangaConfig {
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
    fn fetch(&self) -> Option<ResponseType>;
}

impl Response for AnimeConfig {
    fn new(argument: Argument) -> AnimeConfig {
        AnimeConfig {
            argument,
            id_query: FETCH_ANIME_BY_ID.to_string(),
            search_query: FETCH_ANIME.to_string(),
        }
    }

    // TODO: Move parts to default implementation to make it more reusable?
    fn fetch(&self) -> Option<ResponseType> {
        let response = match &self.argument {
            Argument::Id(value) => {
                let fetched_data = fetch_by_id(self.id_query.clone(), *value);
                let fetch_response: AnimeIdResponse = serde_json::from_str(&fetched_data).unwrap();
                info!("Deserialized response: {:#?}", fetch_response);
                fetch_response.data.unwrap().media
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
        };

        response.map(ResponseType::Anime)
    }
}

// TODO: Figure out whats common and can be moved to make it reusable
impl Response for MangaConfig {
    fn new(argument: Argument) -> MangaConfig {
        MangaConfig {
            argument,
            id_query: FETCH_MANGA_BY_ID.to_string(),
            search_query: FETCH_MANGA.to_string(),
        }
    }

    // TODO: Move parts to default implementation to make it more reusable?
    fn fetch(&self) -> Option<ResponseType> {
        let response = match &self.argument {
            Argument::Id(value) => {
                let fetched_data = fetch_by_id(self.id_query.clone(), *value);
                let fetch_response: MangaIdResponse = serde_json::from_str(&fetched_data).unwrap();
                info!("Deserialized response: {:#?}", fetch_response);
                fetch_response.data.unwrap().media
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
        };

        response.map(ResponseType::Manga)
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
