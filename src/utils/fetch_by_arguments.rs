use crate::utils::requests::anilist::send_request;

use serde_json::json;
use tracing::info;
use wana_kana::{ConvertJapanese, IsJapaneseStr};

pub fn fetch_by_id(query: String, id: u32) -> String {
    let json = json!({"query": query, "variables": {"id":id}});
    let result: String = send_request(json);

    info!("Fetched By ID: {:#?}", id);

    result
}

pub fn fetch_by_name(query: String, name: String) -> String {
    let searchable_name = match name.is_japanese() {
        true => name.clone().to_romaji(),
        false => name.clone(),
    };
    let json = json!({"query": query, "variables": {"search":searchable_name}});
    let result: String = send_request(json);

    info!("User input Name: {:#?}", name);
    info!("Fetched By Name: {:#?}", searchable_name);

    result
}
