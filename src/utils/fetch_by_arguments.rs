use crate::utils::requests::anilist::{AniListRequestError, send_request};

use serde_json::json;
use tracing::info;
use wana_kana::{ConvertJapanese, IsJapaneseStr};

pub fn fetch_by_id(query: String, id: u32) -> Result<String, AniListRequestError> {
    let json = json!({"query": query, "variables": {"id":id}});
    let result = send_request(json)?;

    info!("Fetched By ID: {:#?}", id);

    Ok(result)
}

pub fn fetch_by_name(query: String, name: String) -> Result<String, AniListRequestError> {
    let searchable_name = if name.as_str().is_japanese() {
        name.to_romaji()
    } else {
        name.clone()
    };
    let json = json!({"query": query, "variables": {"search":searchable_name}});
    let result = send_request(json)?;

    info!("User input Name: {:#?}", name);
    info!("Fetched By Name: {:#?}", searchable_name);

    Ok(result)
}
