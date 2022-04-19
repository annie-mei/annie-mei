use crate::commands::anime::fetchers::fetch_by_id::fetch_by_id;
use crate::commands::anime::fetchers::fetch_by_name::fetch_by_name;

use tokio::task;

enum Argument {
    Id(u32),
    Search(String),
}

// TODO: DIfferent fetchers for AniList and MAL
// MAL has song data
impl Argument {
    fn fetch(&self) -> serde_json::Value {
        let fetched_data = match self {
            Self::Id(value) => fetch_by_id(*value),
            Self::Search(value) => fetch_by_name(value.to_string()),
        };
        fetched_data
    }
}

fn return_argument(arg: &str) -> Argument {
    let result = match arg.parse::<u32>() {
        Ok(id) => Argument::Id(id),
        Err(_e) => Argument::Search(arg.to_string()),
    };
    result
}

#[tokio::main]
pub async fn fetcher(mut args: serenity::framework::standard::Args) -> serde_json::Value {
    args.single::<String>().unwrap();
    let args = args.remains().unwrap();

    let argument = return_argument(args);
    let result = task::spawn_blocking(move || argument.fetch())
        .await
        .expect("Fetching Panicked");
    println!("{:#?}", result);

    result
}
