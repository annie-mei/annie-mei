use tracing::instrument;

/// Maximum character length for search inputs (AniList, MAL, etc.).
const MAX_SEARCH_LENGTH: usize = 255;

/// Maximum character length for username inputs (register command).
const MAX_USERNAME_LENGTH: usize = 20;

#[derive(Debug, PartialEq)]
pub enum ValidationError {
    Empty,
    TooLong { max: usize, actual: usize },
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ValidationError::Empty => write!(f, "Input must not be empty"),
            ValidationError::TooLong { max, actual } => {
                write!(
                    f,
                    "Input is too long ({actual} characters, max {max} allowed)"
                )
            }
        }
    }
}

impl std::error::Error for ValidationError {}

/// Validate a search term (anime/manga name or ID string).
///
/// Uses `.chars().count()` for character-based length (not byte-based),
/// so multi-byte UTF-8 characters like Japanese text are counted correctly.
#[instrument(name = "validation.search_term", fields(input_len = input.chars().count()))]
pub fn validate_search_term(input: &str) -> Result<(), ValidationError> {
    validate_length(input, MAX_SEARCH_LENGTH)
}

/// Validate a username (AniList username for the register command).
#[instrument(name = "validation.username", fields(input_len = input.chars().count()))]
pub fn validate_username(input: &str) -> Result<(), ValidationError> {
    validate_length(input, MAX_USERNAME_LENGTH)
}

#[instrument(name = "validation.validate_length", skip(input), fields(max_length = max_length))]
fn validate_length(input: &str, max_length: usize) -> Result<(), ValidationError> {
    if input.trim().is_empty() {
        return Err(ValidationError::Empty);
    }

    let char_count = input.chars().count();

    if char_count > max_length {
        return Err(ValidationError::TooLong {
            max: max_length,
            actual: char_count,
        });
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_search_term() {
        assert!(validate_search_term("One Piece").is_ok());
    }

    #[test]
    fn valid_search_term_numeric_id() {
        assert!(validate_search_term("21").is_ok());
    }

    #[test]
    fn empty_search_term_rejected() {
        assert_eq!(validate_search_term(""), Err(ValidationError::Empty));
    }

    #[test]
    fn search_term_at_max_length() {
        let input = "a".repeat(MAX_SEARCH_LENGTH);
        assert!(validate_search_term(&input).is_ok());
    }

    #[test]
    fn search_term_exceeds_max_length() {
        let input = "a".repeat(MAX_SEARCH_LENGTH + 1);
        assert!(matches!(
            validate_search_term(&input),
            Err(ValidationError::TooLong { .. })
        ));
    }

    #[test]
    fn unicode_search_term_counts_characters_not_bytes() {
        // 5 Japanese characters, well under the limit
        let input = "ワンピース";
        assert_eq!(input.chars().count(), 5);
        assert!(input.len() > 5); // bytes > chars for multibyte
        assert!(validate_search_term(input).is_ok());
    }

    #[test]
    fn valid_username() {
        assert!(validate_username("InfernapeXavier").is_ok());
    }

    #[test]
    fn empty_username_rejected() {
        assert_eq!(validate_username(""), Err(ValidationError::Empty));
    }

    #[test]
    fn username_at_max_length() {
        let input = "a".repeat(MAX_USERNAME_LENGTH);
        assert!(validate_username(&input).is_ok());
    }

    #[test]
    fn username_exceeds_max_length() {
        let input = "a".repeat(MAX_USERNAME_LENGTH + 1);
        assert!(matches!(
            validate_username(&input),
            Err(ValidationError::TooLong { .. })
        ));
    }

    #[test]
    fn unicode_username_counts_characters_not_bytes() {
        // 4 characters, under the username limit
        let input = "テスト太";
        assert_eq!(input.chars().count(), 4);
        assert!(validate_username(input).is_ok());
    }

    #[test]
    fn whitespace_only_search_term_rejected() {
        assert_eq!(validate_search_term("   "), Err(ValidationError::Empty));
    }

    #[test]
    fn whitespace_only_username_rejected() {
        assert_eq!(validate_username("  \t  "), Err(ValidationError::Empty));
    }
}
