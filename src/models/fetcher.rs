use crate::commands::{
    anime::queries::{FETCH_ANIME, FETCH_ANIME_BY_ID},
    manga::queries::{FETCH_MANGA, FETCH_MANGA_BY_ID},
};
use crate::models::{
    id_response::FetchResponse as IdResponse,
    media_list_response::FetchResponse as MediaListResponse, media_type::MediaType as Type,
    transformers::Transformers,
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
    fn get_argument(&self) -> &Argument;
    fn get_id_query(&self) -> String;
    fn get_search_query(&self) -> String;

    fn fetch<
        T: serde::de::DeserializeOwned + Transformers + std::fmt::Debug + std::clone::Clone,
    >(
        &self,
        media_type: Type,
    ) -> Option<T> {
        let response = match self.get_argument() {
            Argument::Id(value) => {
                let fetched_data = fetch_by_id(self.get_id_query(), *value);
                let fetch_response: IdResponse<T> = serde_json::from_str(&fetched_data).unwrap();
                info!("Deserialized response: {:#?}", fetch_response);
                fetch_response.data.unwrap().media
            }
            Argument::Search(value) => {
                let fetched_data = fetch_by_name(self.get_search_query(), value.to_string());
                let fetch_response: MediaListResponse<T> =
                    serde_json::from_str(&fetched_data).unwrap();
                info!("Deserialized response: {:#?}", fetch_response);
                let result = fetch_response.fuzzy_match(value, media_type);
                info!("Fuzzy Response: {:#?}", result);
                result
            }
        };

        response
    }
}

impl Response for AnimeConfig {
    fn new(argument: Argument) -> AnimeConfig {
        AnimeConfig {
            argument,
            id_query: FETCH_ANIME_BY_ID.to_string(),
            search_query: FETCH_ANIME.to_string(),
        }
    }

    fn get_argument(&self) -> &Argument {
        &self.argument
    }

    fn get_id_query(&self) -> String {
        self.id_query.to_owned()
    }

    fn get_search_query(&self) -> String {
        self.search_query.to_owned()
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

    fn get_argument(&self) -> &Argument {
        &self.argument
    }

    fn get_id_query(&self) -> String {
        self.id_query.to_owned()
    }

    fn get_search_query(&self) -> String {
        self.search_query.to_owned()
    }
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
