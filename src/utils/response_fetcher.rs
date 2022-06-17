use crate::models::{
    anilist_anime::Anime,
    fetcher::{AnimeResponse, Argument, Response},
    media_type::MediaType as Type,
};
use tracing::info;

fn return_argument(arg: &str) -> Argument {
    match arg.parse::<u32>() {
        Ok(id) => Argument::Id(id),
        Err(_e) => Argument::Search(arg.to_string()),
    }
}

pub fn fetcher(_media_type: Type, mut args: serenity::framework::standard::Args) -> Option<Anime> {
    // Skips over the first arg because this is the command name
    args.single::<String>().unwrap();

    let args = args.remains().unwrap();
    info!("Found Args: {:#?}", args);

    let argument = return_argument(args);
    let anime_response: AnimeResponse = Response::new(argument);
    anime_response.fetch()
}
