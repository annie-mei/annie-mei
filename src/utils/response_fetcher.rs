use crate::models::{
    fetcher::{AnimeConfig, Argument, MangaConfig, Response},
    media_type::MediaType as Type,
    transformers::Transformers,
};
use serenity::all::CommandDataOptionValue;
use tracing::{info, instrument, warn};

fn strip_quotes(string: &str) -> String {
    string.replace('"', "")
}

#[instrument(name = "fetcher.return_argument", skip(arg))]
fn return_argument(arg: CommandDataOptionValue) -> Option<Argument> {
    let val = if let CommandDataOptionValue::String(name) = arg {
        name
    } else {
        warn!("Invalid argument type for fetcher; expected String");
        return None;
    };

    match val.parse::<u32>() {
        Ok(id) if id > 0 => Some(Argument::Id(id)),
        Err(_) => {
            let val = strip_quotes(&val);
            Some(Argument::Search(val))
        }
        Ok(_) => {
            let val = strip_quotes(&val);
            Some(Argument::Search(val))
        }
    }
}

#[instrument(name = "fetcher.fetch", skip(arg), fields(media_type = ?media_type))]
pub fn fetcher<
    T: serde::de::DeserializeOwned + Transformers + std::fmt::Debug + std::clone::Clone,
>(
    media_type: Type,
    arg: CommandDataOptionValue,
) -> Option<T> {
    info!("Fetcher found arg: {:#?}", arg);
    let argument = return_argument(arg)?;

    match media_type {
        Type::Anime => {
            let anime_response: AnimeConfig = Response::new(argument);
            anime_response.fetch::<T>(media_type)
        }
        Type::Manga => {
            let manga_response: MangaConfig = Response::new(argument);
            manga_response.fetch::<T>(media_type)
        }
    }
}
