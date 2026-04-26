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

/// Redacts credentials from URLs found anywhere in the input string.
///
/// This function finds all URLs embedded in the text and redacts any
/// usernames or passwords they contain. Non-URL text is preserved.
///
/// # Arguments
///
/// * `input` - A string that may contain URLs with credentials
///
/// # Returns
///
/// The input string with any URL credentials redacted. URLs without
/// credentials and non-URL text are preserved unchanged.
///
/// # Example
///
/// ```ignore
/// let text = "Error connecting to postgres://user:pass@localhost:5432/db: connection refused";
/// let redacted = redact_url_credentials(text);
/// assert_eq!(redacted, "Error connecting to postgres://REDACTED_USERNAME:REDACTED_PASSWORD@localhost:5432/db: connection refused");
/// ```
pub fn redact_url_credentials(input: &str) -> String {
    use linkify::{LinkFinder, LinkKind};

    // First, try to parse the entire input as a URL (handles standalone URLs
    // and URLs with percent-encoded characters that linkify might truncate)
    if let Ok(parsed) = url::Url::parse(input)
        && (!parsed.username().is_empty() || parsed.password().is_some())
    {
        return redact_single_url(input);
    }

    let mut finder = LinkFinder::new();
    finder.kinds(&[LinkKind::Url]);

    let links: Vec<_> = finder.links(input).collect();

    if links.is_empty() {
        return input.to_string();
    }

    let mut result = String::with_capacity(input.len());
    let mut last_end = 0;

    for link in links {
        // Append text before this link
        result.push_str(&input[last_end..link.start()]);

        // Try to parse and redact the URL
        let url_str = link.as_str();
        let redacted = redact_single_url(url_str);
        result.push_str(&redacted);

        last_end = link.end();
    }

    // Append any remaining text after the last link
    result.push_str(&input[last_end..]);

    result
}

/// Redacts credentials from a single URL string.
fn redact_single_url(url_str: &str) -> String {
    match url::Url::parse(url_str) {
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
                url_str.to_string()
            }
        }
        Err(_) => url_str.to_string(),
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
mod tests;
