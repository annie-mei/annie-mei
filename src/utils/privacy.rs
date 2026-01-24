//! Privacy utilities for PII handling and redaction.
//!
//! This module provides utilities for hashing user IDs and redacting
//! sensitive information from logs and error reports.

use std::env;
use std::fmt;

use crate::utils::statics::USERID_HASH_SALT;

/// A hashed user ID that can be safely logged without exposing PII.
///
/// The hash is a 16-character hex string (64 bits) derived from the
/// Discord user ID using blake3 with a secret salt.
#[derive(Clone, PartialEq, Eq)]
pub struct HashedUserId(String);

impl HashedUserId {
    /// Returns the hash as a string slice.
    pub fn as_str(&self) -> &str {
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

    let input = format!("{}{}", salt, user_id);
    let hash = blake3::hash(input.as_bytes());
    let hex = hash.to_hex();

    // Truncate to 16 characters (64 bits)
    HashedUserId(hex[..16].to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    fn with_salt<F, R>(salt: &str, f: F) -> R
    where
        F: FnOnce() -> R,
    {
        // SAFETY: Tests run single-threaded with --test-threads=1,
        // so no data races from env var modification.
        unsafe {
            env::set_var(USERID_HASH_SALT, salt);
        }
        let result = f();
        unsafe {
            env::remove_var(USERID_HASH_SALT);
        }
        result
    }

    #[test]
    fn test_hash_determinism() {
        with_salt("test_salt_123", || {
            let hash1 = hash_user_id(123456789012345678);
            let hash2 = hash_user_id(123456789012345678);
            assert_eq!(hash1, hash2, "Same input should produce same hash");
        });
    }

    #[test]
    fn test_hash_length() {
        with_salt("test_salt_123", || {
            let hash = hash_user_id(123456789012345678);
            assert_eq!(hash.as_str().len(), 16, "Hash should be 16 characters");
        });
    }

    #[test]
    fn test_different_users_different_hashes() {
        with_salt("test_salt_123", || {
            let hash1 = hash_user_id(123456789012345678);
            let hash2 = hash_user_id(987654321098765432);
            assert_ne!(hash1, hash2, "Different users should have different hashes");
        });
    }

    #[test]
    fn test_different_salts_different_hashes() {
        let user_id = 123456789012345678;

        let hash1 = with_salt("salt_one", || hash_user_id(user_id));
        let hash2 = with_salt("salt_two", || hash_user_id(user_id));

        assert_ne!(
            hash1, hash2,
            "Different salts should produce different hashes"
        );
    }

    #[test]
    fn test_display_and_debug() {
        with_salt("test_salt_123", || {
            let hash = hash_user_id(123456789012345678);
            let display = format!("{}", hash);
            let debug = format!("{:?}", hash);

            assert_eq!(display.len(), 16);
            assert!(debug.contains("HashedUserId("));
        });
    }
}
