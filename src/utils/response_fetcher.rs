use crate::models::{
    fetcher::{AnimeConfig, Argument, MangaConfig, Response},
    media_type::MediaType as Type,
    transformers::Transformers,
};
use serenity::model::prelude::interaction::application_command::CommandDataOptionValue::{
    self, String as StringData,
};
use tracing::info;

fn strip_quotes(string: &str) -> String {
    string.replace("\"", "")
}

fn return_argument(arg: CommandDataOptionValue) -> Argument {
    let val = match arg {
        StringData(name) => name,
        _ => panic!("Invalid argument type"),
    };

    match val.parse::<u32>() {
        Ok(id) => Argument::Id(id),
        Err(_) => {
            let val = strip_quotes(&val);
            Argument::Search(val)
        }
    }
}

pub fn fetcher<
    T: serde::de::DeserializeOwned + Transformers + std::fmt::Debug + std::clone::Clone,
>(
    media_type: Type,
    arg: CommandDataOptionValue,
) -> Option<T> {
    info!("Fetcher found arg: {:#?}", arg);
    let argument = return_argument(arg);

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
