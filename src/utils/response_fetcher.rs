use crate::models::{
    anilist_character::Character,
    anilist_common::TitleVariant,
    fetcher::{
        AnimeConfig, Argument, CharacterConfig, MangaConfig, Response, fetch, fetch_character,
    },
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
) -> Option<(T, TitleVariant)> {
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

#[instrument(name = "fetcher.fetch_character", skip(arg))]
pub async fn character_fetcher(arg: CommandDataOptionValue) -> Option<Character> {
    info!("Character fetcher found arg: {:#?}", arg);
    let argument = return_argument(arg)?;
    let character_response: CharacterConfig = Response::new(argument);
    fetch_character(&character_response).await
}
