use crate::models::{
    fetcher::{AnimeConfig, Argument, MangaConfig, Response},
    media_type::MediaResponse,
    media_type::MediaType as Type,
};
use tracing::info;

fn return_argument(arg: &str) -> Argument {
    match arg.parse::<u32>() {
        Ok(id) => Argument::Id(id),
        Err(_e) => Argument::Search(arg.to_string()),
    }
}

pub fn fetcher(
    media_type: Type,
    mut args: serenity::framework::standard::Args,
) -> Option<MediaResponse> {
    // Skips over the first arg because this is the command name
    args.single::<String>().unwrap();

    let args = args.remains().unwrap();
    info!("Found Args: {:#?}", args);

    let argument = return_argument(args);

    match media_type {
        Type::Anime => {
            let anime_response: AnimeConfig = Response::new(argument);
            anime_response.fetch()
        }
        Type::Manga => {
            let manga_response: MangaConfig = Response::new(argument);
            manga_response.fetch()
        }
    }
}
