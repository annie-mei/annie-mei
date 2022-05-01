use serde_json::json;
use tracing::info;

use crate::utils::anilist::send_request;

pub mod queries;

pub fn fetch_by_id(query: String, id: u32) -> String {
    let json = json!({"query": query, "variables": {"id":id}});
    let result: String = send_request(json);

    info!("Fetched By ID: {:#?}", id);

    result
}

pub fn fetch_by_name(query: String, name: String) -> String {
    let json = json!({"query": query, "variables": {"search":name}});
    let result: String = send_request(json);

    info!("Fetched By Name: {:#?}", name);

    result
}
