use crate::{
    commands::anime::fetcher::fetcher as anime_fetcher, models::mal_response::MalResponse,
    utils::my_anime_list,
};
use tracing::info;

pub fn fetcher(args: serenity::framework::standard::Args) -> Option<MalResponse> {
    let anime_response = anime_fetcher(args);
    let anime = anime_response.as_ref().unwrap();
    let mal_fetcher_response: String = my_anime_list::send_request(anime.get_mal_id());
    let mal_response: MalResponse = serde_json::from_str(&mal_fetcher_response).unwrap();

    info!("Mal Response: {:#?}", mal_response);
    Some(mal_response)
}
