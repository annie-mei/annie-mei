use std::env;
use std::time::Duration;

use reqwest::blocking::Client;
use tracing::{info, instrument};

use crate::utils::statics::MAL_CLIENT_ID;

#[derive(Debug)]
pub enum MalRequestError {
    MissingClientId,
    ClientBuild(String),
    RequestFailed(String),
    NonSuccessStatus { status: u16, body: String },
    ResponseBodyReadFailed(String),
}

impl std::fmt::Display for MalRequestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MalRequestError::MissingClientId => {
                write!(f, "MAL_CLIENT_ID environment variable is not set")
            }
            MalRequestError::ClientBuild(error) => {
                write!(f, "Failed to build MAL HTTP client: {error}")
            }
            MalRequestError::RequestFailed(error) => {
                write!(f, "Failed to call MAL API: {error}")
            }
            MalRequestError::NonSuccessStatus { status, body } => {
                write!(f, "MAL API returned non-success status {status}: {body}")
            }
            MalRequestError::ResponseBodyReadFailed(error) => {
                write!(f, "Failed to read MAL response body: {error}")
            }
        }
    }
}

impl std::error::Error for MalRequestError {}

const MY_ANIME_LIST_BASE: &str = "https://api.myanimelist.net/v2";
const FIELDS_TO_FETCH: [&str; 3] = ["id", "opening_themes", "ending_themes"];
const MAL_TIMEOUT_SECS: u64 = 10;

#[instrument(name = "http.mal.build_url", fields(mal_id = mal_id))]
fn build_mal_url(mal_id: u32) -> String {
    let mal_url = format!(
        "{MY_ANIME_LIST_BASE}/anime/{mal_id}?fields={}",
        FIELDS_TO_FETCH.join(",")
    );

    info!("Sent MAL Request to URL: {mal_url:#?}");
    mal_url
}

#[instrument(name = "http.mal.send_request", skip_all, fields(mal_id = mal_id))]
pub fn send_request(mal_id: u32) -> Result<String, MalRequestError> {
    let mal_client_id = env::var(MAL_CLIENT_ID).map_err(|_| MalRequestError::MissingClientId)?;

    let client = Client::builder()
        .timeout(Duration::from_secs(MAL_TIMEOUT_SECS))
        .build()
        .map_err(|error| MalRequestError::ClientBuild(error.to_string()))?;

    let response = client
        .get(build_mal_url(mal_id))
        .header("X-MAL-CLIENT-ID", mal_client_id)
        .send()
        .map_err(|error| MalRequestError::RequestFailed(error.to_string()))?;

    let status = response.status();
    let body = response
        .text()
        .map_err(|error| MalRequestError::ResponseBodyReadFailed(error.to_string()))?;

    if !status.is_success() {
        return Err(MalRequestError::NonSuccessStatus {
            status: status.as_u16(),
            body,
        });
    }

    Ok(body)
}
