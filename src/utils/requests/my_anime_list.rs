use std::env;
use std::sync::LazyLock;
use std::time::Duration;

use reqwest::Client;
use tracing::{info, instrument};

use crate::utils::statics::MAL_CLIENT_ID;

#[derive(Debug)]
pub enum MalRequestError {
    ClientBuild(String),
    RequestFailed(String),
    NonSuccessStatus { status: u16, body: String },
    ResponseBodyReadFailed(String),
}

impl std::fmt::Display for MalRequestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
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

static MAL_CLIENT: LazyLock<Result<Client, String>> = LazyLock::new(|| {
    Client::builder()
        .timeout(Duration::from_secs(MAL_TIMEOUT_SECS))
        .build()
        .map_err(|error| error.to_string())
});

#[instrument(name = "http.mal.client", level = "trace")]
fn get_client() -> Result<&'static Client, MalRequestError> {
    match &*MAL_CLIENT {
        Ok(client) => Ok(client),
        Err(error) => Err(MalRequestError::ClientBuild(error.clone())),
    }
}

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
pub async fn send_request(mal_id: u32) -> Result<String, MalRequestError> {
    let mal_client_id = env::var(MAL_CLIENT_ID).expect("Expected MAL_CLIENT_ID in the environment");

    let client = get_client()?;

    let response = client
        .get(build_mal_url(mal_id))
        .header("X-MAL-CLIENT-ID", mal_client_id)
        .send()
        .await
        .map_err(|error| MalRequestError::RequestFailed(error.to_string()))?;

    let status = response.status();
    let body = response
        .text()
        .await
        .map_err(|error| MalRequestError::ResponseBodyReadFailed(error.to_string()))?;

    if !status.is_success() {
        return Err(MalRequestError::NonSuccessStatus {
            status: status.as_u16(),
            body,
        });
    }

    Ok(body)
}
