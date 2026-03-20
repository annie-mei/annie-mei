use reqwest::blocking::Client;
use serde_json::Value;
use std::time::Duration;
use tracing::instrument;

const ANILIST_TIMEOUT_SECS: u64 = 30;

#[derive(Debug)]
pub enum AniListRequestError {
    ClientBuildFailed(String),
    RequestFailed(String),
    NonSuccessStatus { status: u16, body: String },
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
            AniListRequestError::NonSuccessStatus { status, body } => {
                write!(
                    f,
                    "AniList API returned non-success status: status={status}, body_len={}",
                    body.len()
                )
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
        .map_err(|error| AniListRequestError::RequestFailed(error.to_string()))?;

    let status = response.status();

    let body = response
        .text()
        .map_err(|error| AniListRequestError::ResponseBodyReadFailed(error.to_string()))?;

    if !status.is_success() {
        return Err(AniListRequestError::NonSuccessStatus {
            status: status.as_u16(),
            body,
        });
    }

    Ok(body)
}
