use std::sync::LazyLock;
use std::time::Duration;

use reqwest::Client;
use serde_json::Value;
use tracing::instrument;
use url::Url;

use crate::utils::tls::install_rustls_crypto_provider;

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
const ANILIST_ENDPOINT: &str = "https://graphql.anilist.co/";

#[derive(Clone)]
pub struct AniListClient {
    http: Client,
    endpoint: Url,
}

impl AniListClient {
    #[instrument(name = "http.anilist.client.new", skip(endpoint))]
    pub fn new(endpoint: Url, timeout: Duration) -> Result<Self, AniListRequestError> {
        install_rustls_crypto_provider();

        let http = Client::builder()
            .timeout(timeout)
            .build()
            .map_err(|error| AniListRequestError::ClientBuild(error.to_string()))?;

        Ok(Self { http, endpoint })
    }

    #[instrument(name = "http.anilist.client.production")]
    pub fn production() -> Result<Self, AniListRequestError> {
        let endpoint = Url::parse(ANILIST_ENDPOINT)
            .map_err(|error| AniListRequestError::ClientBuild(error.to_string()))?;
        Self::new(endpoint, Duration::from_secs(ANILIST_TIMEOUT_SECS))
    }

    #[instrument(name = "http.anilist.send_request", skip(self, json))]
    pub async fn send_request(&self, json: Value) -> Result<String, AniListRequestError> {
        let response = self
            .http
            .post(self.endpoint.clone())
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
}

static ANILIST_CLIENT: LazyLock<Result<AniListClient, String>> =
    LazyLock::new(|| AniListClient::production().map_err(|error| error.to_string()));

#[instrument(name = "http.anilist.client", level = "trace")]
pub(crate) fn default_client() -> Result<&'static AniListClient, AniListRequestError> {
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
    let client = default_client()?;
    client.send_request(json).await
}
