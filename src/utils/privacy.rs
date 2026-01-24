//! Privacy utilities for PII handling and redaction.
//!
//! This module provides utilities for hashing user IDs and redacting
//! sensitive information from logs and error reports.

use std::collections::BTreeMap;
use std::env;
use std::fmt;

use serde_json::Value;

use crate::utils::statics::USERID_HASH_SALT;

/// A hashed user ID that can be safely logged without exposing PII.
///
/// The hash is a 16-character hex string (64 bits) derived from the
/// Discord user ID using blake3 with a secret salt.
#[derive(Clone, PartialEq, Eq)]
pub struct HashedUserId(String);

impl HashedUserId {
    /// Returns the hash as a string slice.
    #[cfg(test)]
    fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for HashedUserId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl fmt::Debug for HashedUserId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "HashedUserId({})", self.0)
    }
}

/// Hashes a Discord user ID using blake3 with a secret salt.
///
/// The resulting hash is truncated to 16 hex characters (64 bits),
/// which provides sufficient uniqueness for correlation while being
/// compact enough for readable logs.
///
/// # Panics
///
/// Panics if the `USERID_HASH_SALT` environment variable is not set.
///
/// # Example
///
/// ```ignore
/// let hashed = hash_user_id(123456789012345678);
/// println!("User: {}", hashed); // e.g., "a1b2c3d4e5f6g7h8"
/// ```
pub fn hash_user_id(user_id: u64) -> HashedUserId {
    let salt =
        env::var(USERID_HASH_SALT).expect("USERID_HASH_SALT environment variable must be set");

    hash_user_id_with_salt(user_id, &salt)
}

/// Internal function that hashes a user ID with a given salt.
/// Used by tests to avoid unsafe env var manipulation.
fn hash_user_id_with_salt(user_id: u64, salt: &str) -> HashedUserId {
    let input = format!("{}{}", salt, user_id);
    let hash = blake3::hash(input.as_bytes());
    let hex = hash.to_hex();

    // Truncate to 16 characters (64 bits)
    HashedUserId(hex[..16].to_string())
}

/// Redacts credentials from a URL string.
///
/// If the URL contains a username or password, they are replaced with
/// `REDACTED_USERNAME` and `REDACTED_PASSWORD` respectively.
///
/// # Arguments
///
/// * `input` - A string that may contain a URL with credentials
///
/// # Returns
///
/// The input string with any URL credentials redacted. If the input is not
/// a valid URL, it is returned unchanged.
///
/// # Example
///
/// ```ignore
/// let url = "postgres://user:pass@localhost:5432/db";
/// let redacted = redact_url_credentials(url);
/// assert_eq!(redacted, "postgres://REDACTED_USERNAME:REDACTED_PASSWORD@localhost:5432/db");
/// ```
pub fn redact_url_credentials(input: &str) -> String {
    match url::Url::parse(input) {
        Ok(mut parsed_url) => {
            let has_username = !parsed_url.username().is_empty();
            let has_password = parsed_url.password().is_some();

            if has_username || has_password {
                if has_username {
                    let _ = parsed_url.set_username("REDACTED_USERNAME");
                }
                if has_password {
                    let _ = parsed_url.set_password(Some("REDACTED_PASSWORD"));
                }
                parsed_url.to_string()
            } else {
                input.to_string()
            }
        }
        Err(_) => input.to_string(),
    }
}

/// Configures the Sentry scope for a command with privacy-safe user identification.
///
/// This function:
/// - Sets a context with the command name and optional arguments
/// - Sets the user with a hashed user ID (no PII like username)
///
/// # Arguments
///
/// * `command` - The name of the command being executed
/// * `user_id` - The Discord user ID (will be hashed)
/// * `args` - Optional command arguments to include in the context
///
/// # Example
///
/// ```ignore
/// configure_sentry_scope("anime", user.id.get(), Some(json!(arg_str)));
/// ```
pub fn configure_sentry_scope(command: &str, user_id: u64, args: Option<Value>) {
    let hashed_id = hash_user_id(user_id);

    sentry::configure_scope(|scope| {
        let mut context = BTreeMap::new();
        context.insert("Command".to_string(), command.into());
        if let Some(arg_value) = args {
            context.insert("Arg".to_string(), arg_value);
        }
        scope.set_context(command, sentry::protocol::Context::Other(context));
        scope.set_user(Some(sentry::User {
            id: Some(hashed_id.to_string()),
            ..Default::default()
        }));
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_determinism() {
        let hash1 = hash_user_id_with_salt(123456789012345678, "test_salt_123");
        let hash2 = hash_user_id_with_salt(123456789012345678, "test_salt_123");
        assert_eq!(hash1, hash2, "Same input should produce same hash");
    }

    #[test]
    fn test_hash_length() {
        let hash = hash_user_id_with_salt(123456789012345678, "test_salt_123");
        assert_eq!(hash.as_str().len(), 16, "Hash should be 16 characters");
    }

    #[test]
    fn test_different_users_different_hashes() {
        let hash1 = hash_user_id_with_salt(123456789012345678, "test_salt_123");
        let hash2 = hash_user_id_with_salt(987654321098765432, "test_salt_123");
        assert_ne!(hash1, hash2, "Different users should have different hashes");
    }

    #[test]
    fn test_different_salts_different_hashes() {
        let user_id = 123456789012345678;
        let hash1 = hash_user_id_with_salt(user_id, "salt_one");
        let hash2 = hash_user_id_with_salt(user_id, "salt_two");
        assert_ne!(
            hash1, hash2,
            "Different salts should produce different hashes"
        );
    }

    #[test]
    fn test_display_and_debug() {
        let hash = hash_user_id_with_salt(123456789012345678, "test_salt_123");
        let display = format!("{}", hash);
        let debug = format!("{:?}", hash);

        assert_eq!(display.len(), 16);
        assert!(debug.contains("HashedUserId("));
    }

    #[test]
    fn test_redact_url_with_username_and_password() {
        let url = "postgres://myuser:secretpass@localhost:5432/mydb";
        let redacted = redact_url_credentials(url);
        assert_eq!(
            redacted,
            "postgres://REDACTED_USERNAME:REDACTED_PASSWORD@localhost:5432/mydb"
        );
    }

    #[test]
    fn test_redact_url_with_password_only() {
        let url = "redis://:secretpass@localhost:6379";
        let redacted = redact_url_credentials(url);
        assert_eq!(redacted, "redis://:REDACTED_PASSWORD@localhost:6379");
    }

    #[test]
    fn test_redact_url_with_username_only() {
        let url = "ftp://user@example.com/file.txt";
        let redacted = redact_url_credentials(url);
        assert_eq!(redacted, "ftp://REDACTED_USERNAME@example.com/file.txt");
    }

    #[test]
    fn test_redact_url_without_credentials() {
        let url = "https://example.com/api/endpoint";
        let redacted = redact_url_credentials(url);
        assert_eq!(redacted, url);
    }

    #[test]
    fn test_redact_url_invalid_url() {
        let not_a_url = "this is not a url";
        let redacted = redact_url_credentials(not_a_url);
        assert_eq!(redacted, not_a_url);
    }

    #[test]
    fn test_redact_url_with_special_characters() {
        let url = "postgres://user%40domain:p%3Ass%40word@localhost:5432/db";
        let redacted = redact_url_credentials(url);
        assert!(redacted.contains("REDACTED_USERNAME"));
        assert!(redacted.contains("REDACTED_PASSWORD"));
        assert!(!redacted.contains("user%40domain"));
        assert!(!redacted.contains("p%3Ass%40word"));
    }
}
