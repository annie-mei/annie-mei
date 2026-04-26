use std::env;
use std::sync::Arc;

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::Utc;
use hmac::{Hmac, KeyInit, Mac};
use openssl::rand::rand_bytes;
use serde::Serialize;
use serenity::{client::Context, prelude::TypeMapKey};
use sha2::Sha256;
use tracing::instrument;
use url::Url;

use crate::utils::statics::{
    AUTH_SERVICE_BASE_URL, OAUTH_CONTEXT_SIGNING_SECRET, OAUTH_CONTEXT_TTL_SECONDS,
};

const CONTEXT_VERSION: u8 = 1;
const DEFAULT_CONTEXT_TTL_SECONDS: i64 = 300;
const NONCE_BYTES: usize = 16;

#[derive(Clone, PartialEq, Eq)]
pub struct OAuthContextConfig {
    pub auth_service_base_url: String,
    pub signing_secret: String,
    pub ttl_seconds: i64,
}

impl std::fmt::Debug for OAuthContextConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OAuthContextConfig")
            .field("auth_service_base_url", &self.auth_service_base_url)
            .field("signing_secret", &"[REDACTED]")
            .field("ttl_seconds", &self.ttl_seconds)
            .finish()
    }
}

#[derive(Debug)]
pub enum OAuthContextError {
    MissingEnv(&'static str),
    InvalidBaseUrl(url::ParseError),
    /// Base URL must be origin-only so `Url::join("/oauth/...")` does not drop a configured path prefix.
    AuthServiceBaseUrlHasPath,
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
            Self::AuthServiceBaseUrlHasPath => write!(
                f,
                "auth service base URL must not include a path; use the scheme and host only (e.g. https://auth.example.com)"
            ),
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
    let parsed_base =
        Url::parse(&auth_service_base_url).map_err(OAuthContextError::InvalidBaseUrl)?;
    let path = parsed_base.path();
    if !path.is_empty() && path != "/" {
        return Err(OAuthContextError::AuthServiceBaseUrlHasPath);
    }

    let signing_secret = env::var(OAUTH_CONTEXT_SIGNING_SECRET)
        .map_err(|_| OAuthContextError::MissingEnv(OAUTH_CONTEXT_SIGNING_SECRET))?;
    if signing_secret.is_empty() {
        return Err(OAuthContextError::InvalidSecret);
    }

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
    skip(config, discord_user_id, guild_id, interaction_id),
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
    skip(config, nonce, discord_user_id, guild_id, interaction_id),
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

pub struct OAuthContextConfigKey;

impl TypeMapKey for OAuthContextConfigKey {
    type Value = Arc<OAuthContextConfig>;
}

#[instrument(name = "oauth.context.get_from_context", skip(ctx))]
pub async fn get_config_from_context(ctx: &Context) -> Option<Arc<OAuthContextConfig>> {
    let data = ctx.data.read().await;
    data.get::<OAuthContextConfigKey>().cloned()
}

#[cfg(test)]
mod tests;
