use std::sync::LazyLock;
use std::time::Duration;

use reqwest::Client;
use serde_json::Value;
use tracing::instrument;

#[derive(Debug)]
pub enum AniListRequestError {
    ClientBuild(String),
    RequestFailed(String),
    NonSuccessStatus { status: u16, body: String },
    ResponseBodyReadFailed(String),
}

impl std::fmt::Display for AniListRequestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AniListRequestError::ClientBuild(error) => {
                write!(f, "Failed to build AniList HTTP client: {error}")
            }
            AniListRequestError::RequestFailed(error) => {
                write!(f, "Failed to call AniList API: {error}")
            }
            AniListRequestError::NonSuccessStatus { status, body } => {
                write!(
                    f,
                    "AniList API returned non-success status {status}: {body}"
                )
            }
            AniListRequestError::ResponseBodyReadFailed(error) => {
                write!(f, "Failed to read AniList response body: {error}")
            }
        }
    }
}

impl std::error::Error for AniListRequestError {}

const ANILIST_TIMEOUT_SECS: u64 = 10;

static ANILIST_CLIENT: LazyLock<Result<Client, String>> = LazyLock::new(|| {
    Client::builder()
        .timeout(Duration::from_secs(ANILIST_TIMEOUT_SECS))
        .build()
        .map_err(|error| error.to_string())
});

#[instrument(name = "http.anilist.client", level = "trace")]
fn get_client() -> Result<&'static Client, AniListRequestError> {
    match &*ANILIST_CLIENT {
        Ok(client) => Ok(client),
        Err(error) => Err(AniListRequestError::ClientBuild(error.clone())),
    }
}

#[instrument(
    name = "http.anilist.send_request",
    skip(json),
    fields(endpoint = "https://graphql.anilist.co/")
)]
pub async fn send_request(json: Value) -> Result<String, AniListRequestError> {
    let client = get_client()?;

    let response = client
        .post("https://graphql.anilist.co/")
        .header("Content-Type", "application/json")
        .header("Accept", "application/json")
        .body(json.to_string())
        .send()
        .await
        .map_err(|error| AniListRequestError::RequestFailed(error.to_string()))?;

    let status = response.status();
    let body = response
        .text()
        .await
        .map_err(|error| AniListRequestError::ResponseBodyReadFailed(error.to_string()))?;

    if !status.is_success() {
        return Err(AniListRequestError::NonSuccessStatus {
            status: status.as_u16(),
            body,
        });
    }

    Ok(body)
}
