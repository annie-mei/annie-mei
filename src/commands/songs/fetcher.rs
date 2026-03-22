use crate::{
    models::{
        anilist_anime::Anime, mal_response::MalResponse, media_type::MediaType as Type,
        transformers::Transformers,
    },
    utils::{requests::my_anime_list, response_fetcher::fetcher as anime_fetcher},
};

use serenity::all::CommandDataOptionValue;
use tracing::{error, info, instrument};

#[instrument(name = "command.songs.fetcher", skip(args))]
pub fn fetcher(args: CommandDataOptionValue) -> Option<MalResponse> {
    let anime_response: Option<Anime> = anime_fetcher(Type::Anime, args);
    let anime = anime_response?;

    let mal_id = anime.get_mal_id()?;

    let mal_fetcher_response = match my_anime_list::send_request(mal_id) {
        Ok(response) => response,
        Err(err) => {
            error!(error = %err, mal_id = mal_id, "Failed to fetch MAL data for anime");
            return None;
        }
    };

    let mal_response: MalResponse = match serde_json::from_str(&mal_fetcher_response) {
        Ok(response) => response,
        Err(err) => {
            error!(error = %err, mal_id = mal_id, "Failed to deserialize MAL response");
            return None;
        }
    };

    info!("Mal Response: {:#?}", mal_response);
    Some(mal_response)
}
