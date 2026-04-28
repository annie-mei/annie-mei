use crate::utils::requests::anilist::{AniListRequestError, send_request};

use serde_json::json;
use tracing::{info, instrument};
use wana_kana::{ConvertJapanese, IsJapaneseStr};

#[instrument(name = "anilist.fetch_by_id", skip(query), fields(id = id))]
pub async fn fetch_by_id(query: String, id: u32) -> Result<String, AniListRequestError> {
    let json = json!({"query": query, "variables": {"id":id}});
    let result = send_request(json).await?;

    info!("Fetched By ID: {:#?}", id);

    Ok(result)
}

#[instrument(name = "anilist.fetch_by_name", skip(query), fields(name_len = name.len()))]
pub async fn fetch_by_name(query: String, name: String) -> Result<String, AniListRequestError> {
    let searchable_name = if name.as_str().is_japanese() {
        name.to_romaji()
    } else {
        name.clone()
    };
    let json = json!({"query": query, "variables": {"search":searchable_name}});
    let result = send_request(json).await?;

    info!("Fetched By Name: {:#?}", searchable_name);

    Ok(result)
}

#[instrument(name = "anilist.fetch_by_raw_name", skip(query), fields(name_len = name.len()))]
pub async fn fetch_by_raw_name(query: String, name: String) -> Result<String, AniListRequestError> {
    let json = json!({"query": query, "variables": {"search": name}});
    let result = send_request(json).await?;

    info!("Fetched By Raw Name");

    Ok(result)
}
