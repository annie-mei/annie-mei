use super::*;

use serde_json::Value;
use std::{
    env as std_env,
    sync::{Mutex, OnceLock},
};

fn env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

struct EnvVarGuard {
    key: &'static str,
    original_value: Option<String>,
}

impl EnvVarGuard {
    fn set(key: &'static str, value: Option<&str>) -> Self {
        let original_value = std_env::var(key).ok();

        unsafe {
            match value {
                Some(value) => std_env::set_var(key, value),
                None => std_env::remove_var(key),
            }
        }

        Self {
            key,
            original_value,
        }
    }
}

impl Drop for EnvVarGuard {
    fn drop(&mut self) {
        unsafe {
            match &self.original_value {
                Some(value) => std_env::set_var(self.key, value),
                None => std_env::remove_var(self.key),
            }
        }
    }
}

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

#[test]
fn load_context_config_uses_default_ttl_when_env_is_unset() {
    let _lock = env_lock()
        .lock()
        .expect("env test lock should not be poisoned");
    let _base_url = EnvVarGuard::set(AUTH_SERVICE_BASE_URL, Some("https://auth.annie-mei.test"));
    let _secret = EnvVarGuard::set(OAUTH_CONTEXT_SIGNING_SECRET, Some("super-secret"));
    let _ttl = EnvVarGuard::set(OAUTH_CONTEXT_TTL_SECONDS, None);

    let config = load_context_config().expect("config should load with default TTL");

    assert_eq!(config.auth_service_base_url, "https://auth.annie-mei.test");
    assert_eq!(config.signing_secret, "super-secret");
    assert_eq!(config.ttl_seconds, DEFAULT_CONTEXT_TTL_SECONDS);
}

#[test]
fn load_context_config_uses_explicit_ttl_when_present() {
    let _lock = env_lock()
        .lock()
        .expect("env test lock should not be poisoned");
    let _base_url = EnvVarGuard::set(AUTH_SERVICE_BASE_URL, Some("https://auth.annie-mei.test"));
    let _secret = EnvVarGuard::set(OAUTH_CONTEXT_SIGNING_SECRET, Some("super-secret"));
    let _ttl = EnvVarGuard::set(OAUTH_CONTEXT_TTL_SECONDS, Some("120"));

    let config = load_context_config().expect("config should load with explicit TTL");

    assert_eq!(config.ttl_seconds, 120);
}

#[test]
fn load_context_config_rejects_invalid_ttl_values() {
    let _lock = env_lock()
        .lock()
        .expect("env test lock should not be poisoned");
    let _base_url = EnvVarGuard::set(AUTH_SERVICE_BASE_URL, Some("https://auth.annie-mei.test"));
    let _secret = EnvVarGuard::set(OAUTH_CONTEXT_SIGNING_SECRET, Some("super-secret"));
    let _ttl = EnvVarGuard::set(OAUTH_CONTEXT_TTL_SECONDS, Some("0"));

    let result = load_context_config();

    assert!(matches!(result, Err(OAuthContextError::InvalidTtl(value)) if value == "0"));
}

#[test]
fn load_context_config_rejects_invalid_base_url() {
    let _lock = env_lock()
        .lock()
        .expect("env test lock should not be poisoned");
    let _base_url = EnvVarGuard::set(AUTH_SERVICE_BASE_URL, Some("not a url"));
    let _secret = EnvVarGuard::set(OAUTH_CONTEXT_SIGNING_SECRET, Some("super-secret"));
    let _ttl = EnvVarGuard::set(OAUTH_CONTEXT_TTL_SECONDS, None);

    let result = load_context_config();

    assert!(matches!(result, Err(OAuthContextError::InvalidBaseUrl(_))));
}

#[test]
fn load_context_config_rejects_base_url_with_path() {
    let _lock = env_lock()
        .lock()
        .expect("env test lock should not be poisoned");
    let _base_url = EnvVarGuard::set(
        AUTH_SERVICE_BASE_URL,
        Some("https://auth.annie-mei.test/api-prefix"),
    );
    let _secret = EnvVarGuard::set(OAUTH_CONTEXT_SIGNING_SECRET, Some("super-secret"));
    let _ttl = EnvVarGuard::set(OAUTH_CONTEXT_TTL_SECONDS, None);

    let result = load_context_config();

    assert!(matches!(
        result,
        Err(OAuthContextError::AuthServiceBaseUrlHasPath)
    ));
}

#[test]
fn oauth_context_config_debug_redacts_signing_secret() {
    let config = test_config();
    let formatted = format!("{config:?}");
    assert!(formatted.contains("[REDACTED]"));
    assert!(!formatted.contains("super-secret"));
}
