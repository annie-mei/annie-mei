use crate::{
    models::{
        anilist_anime::Anime, mal_response::MalResponse, media_type::MediaType as Type,
        transformers::Transformers,
    },
    utils::{requests::my_anime_list_request, response_fetcher::fetcher as anime_fetcher},
};

use tracing::info;

pub fn fetcher(args: serenity::framework::standard::Args) -> Option<MalResponse> {
    let anime_response: Option<Anime> = anime_fetcher(Type::Anime, args);
    match anime_response {
        None => None,
        Some(anime) => {
            let mal_id = anime.get_mal_id();
            mal_id?;
            let mal_fetcher_response: String = my_anime_list_request::send_request(mal_id.unwrap());
            let mal_response: MalResponse = serde_json::from_str(&mal_fetcher_response).unwrap();

            info!("Mal Response: {:#?}", mal_response);
            Some(mal_response)
        }
    }
}
