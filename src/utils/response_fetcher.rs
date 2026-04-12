use crate::models::{
    fetcher::{AnimeConfig, Argument, MangaConfig, Response, fetch},
    media_type::MediaType as Type,
    transformers::Transformers,
};
use serenity::all::CommandDataOptionValue;
use tracing::{error, info, instrument};

#[instrument(name = "fetcher.strip_quotes")]
fn strip_quotes(string: &str) -> String {
    string.replace('"', "")
}

#[instrument(name = "fetcher.return_argument", skip(arg))]
fn return_argument(arg: CommandDataOptionValue) -> Option<Argument> {
    let val = match arg {
        CommandDataOptionValue::String(name) => name,
        other => {
            error!("Expected String argument, got {:?}", other);
            return None;
        }
    };

    match val.parse::<u32>() {
        Ok(id) => Some(Argument::Id(id)),
        Err(_) => {
            let val = strip_quotes(&val);
            Some(Argument::Search(val))
        }
    }
}

#[instrument(name = "fetcher.fetch", skip(arg), fields(media_type = ?media_type))]
pub async fn fetcher<
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
            fetch::<T>(&anime_response, media_type).await
        }
        Type::Manga => {
            let manga_response: MangaConfig = Response::new(argument);
            fetch::<T>(&manga_response, media_type).await
        }
    }
}
