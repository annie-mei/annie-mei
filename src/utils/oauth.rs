use std::env;

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::Utc;
use hmac::{Hmac, Mac};
use openssl::rand::rand_bytes;
use serde::Serialize;
use sha2::Sha256;
use tracing::instrument;
use url::Url;

use crate::utils::statics::{
    AUTH_SERVICE_BASE_URL, OAUTH_CONTEXT_SIGNING_SECRET, OAUTH_CONTEXT_TTL_SECONDS,
};

const CONTEXT_VERSION: u8 = 1;
const DEFAULT_CONTEXT_TTL_SECONDS: i64 = 300;
const NONCE_BYTES: usize = 16;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OAuthContextConfig {
    pub auth_service_base_url: String,
    pub signing_secret: String,
    pub ttl_seconds: i64,
}

#[derive(Debug)]
pub enum OAuthContextError {
    MissingEnv(&'static str),
    InvalidBaseUrl(url::ParseError),
    InvalidTtl(String),
    InvalidSecret,
    Nonce(openssl::error::ErrorStack),
    Serialize(serde_json::Error),
    UrlJoin(url::ParseError),
}

impl std::fmt::Display for OAuthContextError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingEnv(key) => write!(f, "missing required environment variable {key}"),
            Self::InvalidBaseUrl(err) => write!(f, "invalid auth service base URL: {err}"),
            Self::InvalidTtl(value) => write!(
                f,
                "OAUTH_CONTEXT_TTL_SECONDS must be a positive integer, got {value}"
            ),
            Self::InvalidSecret => write!(f, "invalid OAuth context signing secret"),
            Self::Nonce(err) => write!(f, "failed to generate OAuth nonce: {err}"),
            Self::Serialize(err) => write!(f, "failed to serialize OAuth context payload: {err}"),
            Self::UrlJoin(err) => write!(f, "failed to build OAuth start URL: {err}"),
        }
    }
}

impl std::error::Error for OAuthContextError {}

#[derive(Debug, Serialize)]
struct OAuthContextPayload<'a> {
    v: u8,
    discord_user_id: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    guild_id: Option<&'a str>,
    interaction_id: &'a str,
    nonce: &'a str,
    iat: i64,
    exp: i64,
}

#[instrument(name = "oauth.context.load_config")]
pub fn load_context_config() -> Result<OAuthContextConfig, OAuthContextError> {
    let auth_service_base_url = env::var(AUTH_SERVICE_BASE_URL)
        .map_err(|_| OAuthContextError::MissingEnv(AUTH_SERVICE_BASE_URL))?;
    Url::parse(&auth_service_base_url).map_err(OAuthContextError::InvalidBaseUrl)?;

    let signing_secret = env::var(OAUTH_CONTEXT_SIGNING_SECRET)
        .map_err(|_| OAuthContextError::MissingEnv(OAUTH_CONTEXT_SIGNING_SECRET))?;

    let ttl_seconds = match env::var(OAUTH_CONTEXT_TTL_SECONDS) {
        Ok(value) => value
            .parse::<i64>()
            .ok()
            .filter(|ttl| *ttl > 0)
            .ok_or(OAuthContextError::InvalidTtl(value))?,
        Err(_) => DEFAULT_CONTEXT_TTL_SECONDS,
    };

    Ok(OAuthContextConfig {
        auth_service_base_url,
        signing_secret,
        ttl_seconds,
    })
}

#[instrument(
    name = "oauth.context.build_start_url",
    skip(config),
    fields(discord_user_id_len = discord_user_id.len(), has_guild_id = guild_id.is_some())
)]
pub fn build_oauth_start_url(
    discord_user_id: &str,
    guild_id: Option<&str>,
    interaction_id: &str,
    config: &OAuthContextConfig,
) -> Result<Url, OAuthContextError> {
    let issued_at = Utc::now().timestamp();
    let nonce = generate_nonce()?;

    build_oauth_start_url_with_values(
        discord_user_id,
        guild_id,
        interaction_id,
        issued_at,
        &nonce,
        config,
    )
}

#[instrument(
    name = "oauth.context.build_start_url_with_values",
    skip(config, nonce),
    fields(discord_user_id_len = discord_user_id.len(), has_guild_id = guild_id.is_some())
)]
fn build_oauth_start_url_with_values(
    discord_user_id: &str,
    guild_id: Option<&str>,
    interaction_id: &str,
    issued_at: i64,
    nonce: &str,
    config: &OAuthContextConfig,
) -> Result<Url, OAuthContextError> {
    let payload = OAuthContextPayload {
        v: CONTEXT_VERSION,
        discord_user_id,
        guild_id,
        interaction_id,
        nonce,
        iat: issued_at,
        exp: issued_at + config.ttl_seconds,
    };
    let payload_json = serde_json::to_vec(&payload).map_err(OAuthContextError::Serialize)?;
    let payload_segment = URL_SAFE_NO_PAD.encode(payload_json);
    let signature_segment = sign_payload_segment(&payload_segment, &config.signing_secret)?;
    let ctx = format!("{payload_segment}.{signature_segment}");

    let mut url = Url::parse(&config.auth_service_base_url)
        .map_err(OAuthContextError::InvalidBaseUrl)?
        .join("/oauth/anilist/start")
        .map_err(OAuthContextError::UrlJoin)?;
    url.query_pairs_mut().append_pair("ctx", &ctx);

    Ok(url)
}

#[instrument(name = "oauth.context.generate_nonce")]
fn generate_nonce() -> Result<String, OAuthContextError> {
    let mut bytes = [0u8; NONCE_BYTES];
    rand_bytes(&mut bytes).map_err(OAuthContextError::Nonce)?;
    Ok(URL_SAFE_NO_PAD.encode(bytes))
}

#[instrument(
    name = "oauth.context.sign_payload_segment",
    skip(secret, payload_segment)
)]
fn sign_payload_segment(payload_segment: &str, secret: &str) -> Result<String, OAuthContextError> {
    type HmacSha256 = Hmac<Sha256>;

    let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
        .map_err(|_| OAuthContextError::InvalidSecret)?;
    mac.update(payload_segment.as_bytes());
    Ok(URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;

    use serde_json::Value;

    fn test_config() -> OAuthContextConfig {
        OAuthContextConfig {
            auth_service_base_url: "https://auth.annie-mei.test".to_string(),
            signing_secret: "super-secret".to_string(),
            ttl_seconds: 300,
        }
    }

    #[test]
    fn build_oauth_start_url_uses_contract_shape() {
        let url = build_oauth_start_url_with_values(
            "123456789012345678",
            Some("987654321098765432"),
            "12222333344445555",
            1_711_500_000,
            "bM0XvTa5yT4K0z2yPxtA3A",
            &test_config(),
        )
        .expect("OAuth start URL should build");

        assert_eq!(url.path(), "/oauth/anilist/start");

        let ctx = url
            .query_pairs()
            .find(|(key, _)| key == "ctx")
            .map(|(_, value)| value.into_owned())
            .expect("ctx query parameter should be present");
        let (payload_segment, signature_segment) = ctx
            .split_once('.')
            .expect("ctx should contain payload and signature");

        let payload_json = URL_SAFE_NO_PAD
            .decode(payload_segment)
            .expect("payload should decode as base64url");
        let payload: Value =
            serde_json::from_slice(&payload_json).expect("payload JSON should deserialize");

        assert_eq!(payload["v"], 1);
        assert_eq!(payload["discord_user_id"], "123456789012345678");
        assert_eq!(payload["guild_id"], "987654321098765432");
        assert_eq!(payload["interaction_id"], "12222333344445555");
        assert_eq!(payload["nonce"], "bM0XvTa5yT4K0z2yPxtA3A");
        assert_eq!(payload["iat"], 1_711_500_000);
        assert_eq!(payload["exp"], 1_711_500_300);

        let expected_signature =
            sign_payload_segment(payload_segment, "super-secret").expect("signature should build");
        assert_eq!(signature_segment, expected_signature);
    }

    #[test]
    fn build_oauth_start_url_omits_guild_id_when_not_available() {
        let url = build_oauth_start_url_with_values(
            "123456789012345678",
            None,
            "12222333344445555",
            1_711_500_000,
            "bM0XvTa5yT4K0z2yPxtA3A",
            &test_config(),
        )
        .expect("OAuth start URL should build");

        let ctx = url
            .query_pairs()
            .find(|(key, _)| key == "ctx")
            .map(|(_, value)| value.into_owned())
            .expect("ctx query parameter should be present");
        let (payload_segment, _) = ctx
            .split_once('.')
            .expect("ctx should contain payload and signature");
        let payload_json = URL_SAFE_NO_PAD
            .decode(payload_segment)
            .expect("payload should decode as base64url");
        let payload: Value =
            serde_json::from_slice(&payload_json).expect("payload JSON should deserialize");

        assert!(payload.get("guild_id").is_none());
    }
}
