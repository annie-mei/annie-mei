use reqwest::blocking::Client;
use serde_json::Value;
use std::time::Duration;
use tracing::instrument;

const ANILIST_TIMEOUT_SECS: u64 = 30;

#[derive(Debug)]
pub enum AniListRequestError {
    ClientBuildFailed(String),
    RequestFailed(String),
    NonSuccessStatus(String),
    ResponseBodyReadFailed(String),
}

impl std::fmt::Display for AniListRequestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AniListRequestError::ClientBuildFailed(error) => {
                write!(f, "Failed to build AniList HTTP client: {error}")
            }
            AniListRequestError::RequestFailed(error) => {
                write!(f, "Failed to call AniList API: {error}")
            }
            AniListRequestError::NonSuccessStatus(error) => {
                write!(f, "AniList API returned non-success status: {error}")
            }
            AniListRequestError::ResponseBodyReadFailed(error) => {
                write!(f, "Failed to read AniList response body: {error}")
            }
        }
    }
}

impl std::error::Error for AniListRequestError {}

#[instrument(
    name = "http.anilist.send_request",
    skip(json),
    fields(endpoint = "https://graphql.anilist.co/")
)]
pub fn send_request(json: Value) -> Result<String, AniListRequestError> {
    let client = Client::builder()
        .timeout(Duration::from_secs(ANILIST_TIMEOUT_SECS))
        .build()
        .map_err(|error| AniListRequestError::ClientBuildFailed(error.to_string()))?;
    let response = client
        .post("https://graphql.anilist.co/")
        .header("Content-Type", "application/json")
        .header("Accept", "application/json")
        .body(json.to_string())
        .send()
        .map_err(|error| AniListRequestError::RequestFailed(error.to_string()))?
        .error_for_status()
        .map_err(|error| AniListRequestError::NonSuccessStatus(error.to_string()))?;

    response
        .text()
        .map_err(|error| AniListRequestError::ResponseBodyReadFailed(error.to_string()))
}
