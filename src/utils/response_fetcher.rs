use crate::models::{
    fetcher::{AnimeConfig, Argument, MangaConfig, Response},
    media_type::MediaType as Type,
    transformers::Transformers,
};
use serenity::all::CommandDataOptionValue;
use tracing::{info, instrument, warn};

#[derive(Debug)]
pub enum FetchError {
    InvalidArgumentType {
        expected: &'static str,
        actual: String,
    },
}

impl std::fmt::Display for FetchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FetchError::InvalidArgumentType { expected, actual } => {
                write!(
                    f,
                    "Invalid argument type for fetcher: expected {expected}, got {actual}"
                )
            }
        }
    }
}

impl std::error::Error for FetchError {}

fn strip_quotes(string: &str) -> String {
    string.replace('"', "")
}

#[instrument(name = "fetcher.return_argument", skip(arg))]
fn return_argument(arg: CommandDataOptionValue) -> Result<Argument, FetchError> {
    let actual_type = format!("{:?}", arg.kind());

    let val = if let CommandDataOptionValue::String(name) = arg {
        name
    } else {
        return Err(FetchError::InvalidArgumentType {
            expected: "String",
            actual: actual_type,
        });
    };

    match val.parse::<u32>() {
        Ok(id) if id > 0 => Ok(Argument::Id(id)),
        Err(_) => {
            let val = strip_quotes(&val);
            Ok(Argument::Search(val))
        }
        Ok(_) => {
            let val = strip_quotes(&val);
            Ok(Argument::Search(val))
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
    let argument = match return_argument(arg) {
        Ok(argument) => argument,
        Err(error) => {
            warn!(error = %error, "Failed to build fetch argument");
            return None;
        }
    };

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
