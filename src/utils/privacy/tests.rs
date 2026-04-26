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
fn test_redact_standalone_url_with_percent_encoding() {
    // Standalone URLs with percent-encoded characters should be handled
    // even if linkify truncates them, because we try direct URL parsing first
    let url = "postgres://admin:p%23ss%21word@localhost:5432/db";
    let redacted = redact_url_credentials(url);
    assert!(redacted.contains("REDACTED_USERNAME"));
    assert!(redacted.contains("REDACTED_PASSWORD"));
    assert!(!redacted.contains("admin"));
    assert!(!redacted.contains("p%23ss%21word"));
}

#[test]
fn test_redact_embedded_url_in_error_message() {
    let text = "Error connecting to postgres://user:pass@localhost:5432/db: connection refused";
    let redacted = redact_url_credentials(text);
    assert_eq!(
        redacted,
        "Error connecting to postgres://REDACTED_USERNAME:REDACTED_PASSWORD@localhost:5432/db: connection refused"
    );
}

#[test]
fn test_redact_multiple_embedded_urls() {
    let text = "Failed to connect to postgres://admin:secret@db.example.com/prod and redis://:password@cache.example.com:6379";
    let redacted = redact_url_credentials(text);
    assert!(
        redacted.contains("postgres://REDACTED_USERNAME:REDACTED_PASSWORD@db.example.com/prod")
    );
    assert!(redacted.contains("redis://:REDACTED_PASSWORD@cache.example.com:6379"));
    assert!(!redacted.contains("admin"));
    assert!(!redacted.contains("secret"));
    assert!(!redacted.contains("password"));
}

#[test]
fn test_redact_url_preserves_surrounding_text() {
    let text = "prefix https://user:pass@example.com suffix";
    let redacted = redact_url_credentials(text);
    assert!(redacted.starts_with("prefix "));
    assert!(redacted.ends_with(" suffix"));
    assert!(redacted.contains("REDACTED_USERNAME"));
    assert!(redacted.contains("REDACTED_PASSWORD"));
}

#[test]
fn test_redact_url_no_credentials_in_embedded() {
    let text = "Check out https://example.com/page for more info";
    let redacted = redact_url_credentials(text);
    assert_eq!(redacted, text);
}

#[test]
fn test_redact_url_mixed_with_and_without_credentials() {
    let text = "Connect to postgres://user:pass@localhost/db or visit https://docs.example.com";
    let redacted = redact_url_credentials(text);
    assert!(redacted.contains("postgres://REDACTED_USERNAME:REDACTED_PASSWORD@localhost/db"));
    assert!(redacted.contains("https://docs.example.com"));
    assert!(!redacted.contains("user:pass"));
}
