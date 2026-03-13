use serenity::all::{CommandDataOption, CommandDataOptionValue};
use tracing::instrument;

pub const MAX_SEARCH_INPUT_LEN: usize = 100;
pub const MAX_ANILIST_USERNAME_LEN: usize = 50;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SearchInputKind {
    Id,
    Text,
}

impl SearchInputKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            SearchInputKind::Id => "id",
            SearchInputKind::Text => "text",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedSearchInput {
    pub value: String,
    pub kind: SearchInputKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputValidationError {
    MissingRequired {
        option_name: &'static str,
    },
    InvalidType {
        option_name: &'static str,
    },
    EmptyValue {
        option_name: &'static str,
    },
    TooLong {
        option_name: &'static str,
        max_len: usize,
    },
    NumericIdOutOfRange {
        option_name: &'static str,
        max_id: u32,
    },
}

impl InputValidationError {
    pub fn user_message(&self) -> String {
        match self {
            InputValidationError::MissingRequired { option_name } => {
                format!(
                    "Missing required `{option_name}` input. Please provide a value and try again."
                )
            }
            InputValidationError::InvalidType { option_name } => {
                format!("Invalid `{option_name}` input type. Please provide text.")
            }
            InputValidationError::EmptyValue { option_name } => {
                format!("`{option_name}` cannot be empty. Please provide a value and try again.")
            }
            InputValidationError::TooLong {
                option_name,
                max_len,
            } => {
                format!(
                    "`{option_name}` is too long (max {max_len} characters). Please shorten it and try again."
                )
            }
            InputValidationError::NumericIdOutOfRange {
                option_name,
                max_id,
            } => {
                format!(
                    "`{option_name}` numeric ID is out of range. Please use a value between 1 and {max_id}."
                )
            }
        }
    }
}

#[instrument(name = "commands.input.validate_required_string_option", skip(options))]
pub fn validate_required_string_option(
    options: &[CommandDataOption],
    option_name: &'static str,
    max_len: usize,
) -> Result<String, InputValidationError> {
    let Some(option) = options.iter().find(|option| option.name == option_name) else {
        return Err(InputValidationError::MissingRequired { option_name });
    };

    let CommandDataOptionValue::String(raw_value) = &option.value else {
        return Err(InputValidationError::InvalidType { option_name });
    };

    let value = raw_value.trim();

    if value.is_empty() {
        return Err(InputValidationError::EmptyValue { option_name });
    }

    if value.chars().count() > max_len {
        return Err(InputValidationError::TooLong {
            option_name,
            max_len,
        });
    }

    Ok(value.to_string())
}

#[instrument(name = "commands.input.validate_search_option", skip(options))]
pub fn validate_search_option(
    options: &[CommandDataOption],
    option_name: &'static str,
    max_len: usize,
) -> Result<ValidatedSearchInput, InputValidationError> {
    let value = validate_required_string_option(options, option_name, max_len)?;

    if value.chars().all(|character| character.is_ascii_digit()) {
        let id = value.parse::<u64>().ok();
        match id {
            Some(0) | None => {
                return Err(InputValidationError::NumericIdOutOfRange {
                    option_name,
                    max_id: u32::MAX,
                });
            }
            Some(parsed_id) if parsed_id > u32::MAX as u64 => {
                return Err(InputValidationError::NumericIdOutOfRange {
                    option_name,
                    max_id: u32::MAX,
                });
            }
            Some(_) => {
                return Ok(ValidatedSearchInput {
                    value,
                    kind: SearchInputKind::Id,
                });
            }
        }
    }

    Ok(ValidatedSearchInput {
        value,
        kind: SearchInputKind::Text,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn string_option(name: &str, value: &str) -> CommandDataOption {
        serde_json::from_value(json!({
            "name": name,
            "type": 3,
            "value": value,
        }))
        .expect("string command option should deserialize")
    }

    #[test]
    fn required_string_rejects_whitespace_only_input() {
        let options = vec![string_option("search", "   ")];

        let result = validate_required_string_option(&options, "search", MAX_SEARCH_INPUT_LEN);

        assert_eq!(
            result,
            Err(InputValidationError::EmptyValue {
                option_name: "search"
            })
        );
    }

    #[test]
    fn search_option_rejects_zero_id() {
        let options = vec![string_option("search", "0")];

        let result = validate_search_option(&options, "search", MAX_SEARCH_INPUT_LEN);

        assert_eq!(
            result,
            Err(InputValidationError::NumericIdOutOfRange {
                option_name: "search",
                max_id: u32::MAX,
            })
        );
    }

    #[test]
    fn search_option_rejects_id_above_u32_max() {
        let above_u32_max = (u32::MAX as u64 + 1).to_string();
        let options = vec![string_option("search", &above_u32_max)];

        let result = validate_search_option(&options, "search", MAX_SEARCH_INPUT_LEN);

        assert_eq!(
            result,
            Err(InputValidationError::NumericIdOutOfRange {
                option_name: "search",
                max_id: u32::MAX,
            })
        );
    }

    #[test]
    fn required_string_length_uses_character_count_for_multibyte_input() {
        let options = vec![string_option("search", "あいう")];

        let result = validate_required_string_option(&options, "search", 2);

        assert_eq!(
            result,
            Err(InputValidationError::TooLong {
                option_name: "search",
                max_len: 2,
            })
        );
    }

    #[test]
    fn search_option_classifies_numeric_id() {
        let options = vec![string_option("search", "42")];

        let result = validate_search_option(&options, "search", MAX_SEARCH_INPUT_LEN);

        assert_eq!(
            result,
            Ok(ValidatedSearchInput {
                value: "42".to_string(),
                kind: SearchInputKind::Id,
            })
        );
    }

    #[test]
    fn search_option_classifies_text_input() {
        let options = vec![string_option("search", "one piece")];

        let result = validate_search_option(&options, "search", MAX_SEARCH_INPUT_LEN);

        assert_eq!(
            result,
            Ok(ValidatedSearchInput {
                value: "one piece".to_string(),
                kind: SearchInputKind::Text,
            })
        );
    }
}
