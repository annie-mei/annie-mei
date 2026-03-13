use crate::utils::requests::anilist::send_request;

use serde_json::json;
use tracing::{error, info};
use wana_kana::{ConvertJapanese, IsJapaneseStr};

pub fn fetch_by_id(query: String, id: u32) -> Option<String> {
    let json = json!({"query": query, "variables": {"id":id}});
    let result = match send_request(json) {
        Ok(result) => result,
        Err(error) => {
            error!(error = %error, id, "Failed to fetch AniList data by id");
            return None;
        }
    };

    info!("Fetched By ID: {:#?}", id);

    Some(result)
}

pub fn fetch_by_name(query: String, name: String) -> Option<String> {
    let searchable_name = if name.as_str().is_japanese() {
        name.to_romaji()
    } else {
        name.clone()
    };
    let json = json!({"query": query, "variables": {"search":searchable_name}});
    let result = match send_request(json) {
        Ok(result) => result,
        Err(error) => {
            error!(
                error = %error,
                searchable_name = %searchable_name,
                "Failed to fetch AniList data by search term"
            );
            return None;
        }
    };

    info!("User input Name: {:#?}", name);
    info!("Fetched By Name: {:#?}", searchable_name);

    Some(result)
}
