//! Typed settings registry shared by commands and feature code.

use std::fmt;

use tracing::instrument;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingScope {
    Effective,
    User,
    Guild,
}

impl SettingScope {
    pub fn parse(raw: &str) -> Option<Self> {
        match normalize_token(raw).as_str() {
            "effective" => Some(Self::Effective),
            "user" => Some(Self::User),
            "guild" | "server" => Some(Self::Guild),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Effective => "effective",
            Self::User => "user",
            Self::Guild => "guild",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingSource {
    User,
    Guild,
    Default,
}

impl fmt::Display for SettingSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::User => write!(f, "user override"),
            Self::Guild => write!(f, "guild override"),
            Self::Default => write!(f, "default"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingKey {
    TitleDisplay,
    GuildScores,
    AnalyticsPrivacy,
}

pub const ALL_SETTING_KEYS: [SettingKey; 3] = [
    SettingKey::TitleDisplay,
    SettingKey::GuildScores,
    SettingKey::AnalyticsPrivacy,
];

impl SettingKey {
    pub fn parse(raw: &str) -> Option<Self> {
        match normalize_token(raw).as_str() {
            "title_display" | "title" | "titles" => Some(Self::TitleDisplay),
            "guild_scores" | "guild_score" | "scores" => Some(Self::GuildScores),
            "analytics_privacy" | "analytics" | "privacy" => Some(Self::AnalyticsPrivacy),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::TitleDisplay => "title_display",
            Self::GuildScores => "guild_scores",
            Self::AnalyticsPrivacy => "analytics_privacy",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::TitleDisplay => "Title display",
            Self::GuildScores => "Guild scores",
            Self::AnalyticsPrivacy => "Analytics privacy",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Self::TitleDisplay => "Which AniList title variant Annie Mei should prefer.",
            Self::GuildScores => "Whether guild member scores should be shown in media embeds.",
            Self::AnalyticsPrivacy => {
                "Whether analytics should use the standard telemetry path or opt out where supported."
            }
        }
    }

    pub fn default_value(self) -> SettingValue {
        match self {
            Self::TitleDisplay => SettingValue::TitleDisplay(TitleDisplayPreference::Matched),
            Self::GuildScores => SettingValue::GuildScores(GuildScorePreference::Visible),
            Self::AnalyticsPrivacy => {
                SettingValue::AnalyticsPrivacy(AnalyticsPrivacyPreference::Standard)
            }
        }
    }

    pub fn allowed_values(self) -> &'static [&'static str] {
        match self {
            Self::TitleDisplay => &["matched", "romaji", "english", "native"],
            Self::GuildScores => &["visible", "hidden"],
            Self::AnalyticsPrivacy => &["standard", "opted_out"],
        }
    }

    pub fn allowed_values_sentence(self) -> String {
        self.allowed_values().join(", ")
    }

    pub fn parse_value(self, raw: &str) -> Result<SettingValue, SettingValidationError> {
        let normalized = normalize_token(raw);
        let value = match self {
            Self::TitleDisplay => match normalized.as_str() {
                "matched" | "match" | "auto" => {
                    SettingValue::TitleDisplay(TitleDisplayPreference::Matched)
                }
                "romaji" => SettingValue::TitleDisplay(TitleDisplayPreference::Romaji),
                "english" => SettingValue::TitleDisplay(TitleDisplayPreference::English),
                "native" => SettingValue::TitleDisplay(TitleDisplayPreference::Native),
                _ => return Err(SettingValidationError::new(self, raw)),
            },
            Self::GuildScores => match normalized.as_str() {
                "visible" | "show" | "shown" | "enabled" | "on" | "true" => {
                    SettingValue::GuildScores(GuildScorePreference::Visible)
                }
                "hidden" | "hide" | "disabled" | "off" | "false" => {
                    SettingValue::GuildScores(GuildScorePreference::Hidden)
                }
                _ => return Err(SettingValidationError::new(self, raw)),
            },
            Self::AnalyticsPrivacy => match normalized.as_str() {
                "standard" | "enabled" | "on" | "default" => {
                    SettingValue::AnalyticsPrivacy(AnalyticsPrivacyPreference::Standard)
                }
                "opted_out" | "optout" | "private" | "disabled" | "off" => {
                    SettingValue::AnalyticsPrivacy(AnalyticsPrivacyPreference::OptedOut)
                }
                _ => return Err(SettingValidationError::new(self, raw)),
            },
        };

        Ok(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TitleDisplayPreference {
    Matched,
    Romaji,
    English,
    Native,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuildScorePreference {
    Visible,
    Hidden,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnalyticsPrivacyPreference {
    Standard,
    OptedOut,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingValue {
    TitleDisplay(TitleDisplayPreference),
    GuildScores(GuildScorePreference),
    AnalyticsPrivacy(AnalyticsPrivacyPreference),
}

impl SettingValue {
    pub fn key(self) -> SettingKey {
        match self {
            Self::TitleDisplay(_) => SettingKey::TitleDisplay,
            Self::GuildScores(_) => SettingKey::GuildScores,
            Self::AnalyticsPrivacy(_) => SettingKey::AnalyticsPrivacy,
        }
    }

    pub fn as_storage_value(self) -> &'static str {
        match self {
            Self::TitleDisplay(TitleDisplayPreference::Matched) => "matched",
            Self::TitleDisplay(TitleDisplayPreference::Romaji) => "romaji",
            Self::TitleDisplay(TitleDisplayPreference::English) => "english",
            Self::TitleDisplay(TitleDisplayPreference::Native) => "native",
            Self::GuildScores(GuildScorePreference::Visible) => "visible",
            Self::GuildScores(GuildScorePreference::Hidden) => "hidden",
            Self::AnalyticsPrivacy(AnalyticsPrivacyPreference::Standard) => "standard",
            Self::AnalyticsPrivacy(AnalyticsPrivacyPreference::OptedOut) => "opted_out",
        }
    }

    pub fn display_label(self) -> &'static str {
        match self {
            Self::TitleDisplay(TitleDisplayPreference::Matched) => "matched title",
            Self::TitleDisplay(TitleDisplayPreference::Romaji) => "Romaji title",
            Self::TitleDisplay(TitleDisplayPreference::English) => "English title",
            Self::TitleDisplay(TitleDisplayPreference::Native) => "native title",
            Self::GuildScores(GuildScorePreference::Visible) => "show guild scores",
            Self::GuildScores(GuildScorePreference::Hidden) => "hide guild scores",
            Self::AnalyticsPrivacy(AnalyticsPrivacyPreference::Standard) => "standard analytics",
            Self::AnalyticsPrivacy(AnalyticsPrivacyPreference::OptedOut) => {
                "opted out of analytics"
            }
        }
    }
}

impl fmt::Display for SettingValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.display_label())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingValidationError {
    pub setting_key: SettingKey,
    pub raw_value: String,
}

impl SettingValidationError {
    fn new(setting_key: SettingKey, raw_value: &str) -> Self {
        Self {
            setting_key,
            raw_value: raw_value.to_string(),
        }
    }
}

impl fmt::Display for SettingValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "`{}` is not valid for {}. Allowed values: {}.",
            self.raw_value,
            self.setting_key.label(),
            self.setting_key.allowed_values_sentence()
        )
    }
}

impl std::error::Error for SettingValidationError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResolvedSetting {
    pub key: SettingKey,
    pub value: SettingValue,
    pub source: SettingSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScopedSettingValues {
    pub user: Option<SettingValue>,
    pub guild: Option<SettingValue>,
}

#[instrument(name = "settings.resolve", skip(values), fields(setting_key = %key.as_str()))]
pub fn resolve_setting(key: SettingKey, values: ScopedSettingValues) -> ResolvedSetting {
    if let Some(value) = values.user {
        return ResolvedSetting {
            key,
            value,
            source: SettingSource::User,
        };
    }

    if let Some(value) = values.guild {
        return ResolvedSetting {
            key,
            value,
            source: SettingSource::Guild,
        };
    }

    ResolvedSetting {
        key,
        value: key.default_value(),
        source: SettingSource::Default,
    }
}

fn normalize_token(raw: &str) -> String {
    raw.trim().to_ascii_lowercase().replace([' ', '-'], "_")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn setting_keys_parse_canonical_names_and_aliases() {
        assert_eq!(
            SettingKey::parse("title-display"),
            Some(SettingKey::TitleDisplay)
        );
        assert_eq!(SettingKey::parse("scores"), Some(SettingKey::GuildScores));
        assert_eq!(
            SettingKey::parse("privacy"),
            Some(SettingKey::AnalyticsPrivacy)
        );
        assert_eq!(SettingKey::parse("unknown"), None);
    }

    #[test]
    fn setting_values_round_trip_through_storage_strings() {
        for key in ALL_SETTING_KEYS {
            for raw in key.allowed_values() {
                let parsed = key.parse_value(raw).expect("value should parse");
                assert_eq!(parsed.key(), key);
                assert_eq!(
                    key.parse_value(parsed.as_storage_value())
                        .expect("stored value should parse"),
                    parsed
                );
            }
        }
    }

    #[test]
    fn setting_validation_rejects_invalid_values_with_allowed_values() {
        let error = SettingKey::TitleDisplay
            .parse_value("kana")
            .expect_err("invalid title value should fail");

        let message = error.to_string();
        assert!(message.contains("kana"));
        assert!(message.contains("matched, romaji, english, native"));
    }

    #[test]
    fn resolve_setting_prefers_user_then_guild_then_default() {
        let guild = SettingKey::TitleDisplay.parse_value("english").ok();
        let user = SettingKey::TitleDisplay.parse_value("romaji").ok();

        let user_resolved = resolve_setting(
            SettingKey::TitleDisplay,
            ScopedSettingValues { user, guild },
        );
        assert_eq!(user_resolved.source, SettingSource::User);
        assert_eq!(
            user_resolved.value,
            SettingValue::TitleDisplay(TitleDisplayPreference::Romaji)
        );

        let guild_resolved = resolve_setting(
            SettingKey::TitleDisplay,
            ScopedSettingValues { user: None, guild },
        );
        assert_eq!(guild_resolved.source, SettingSource::Guild);
        assert_eq!(
            guild_resolved.value,
            SettingValue::TitleDisplay(TitleDisplayPreference::English)
        );

        let default_resolved = resolve_setting(
            SettingKey::TitleDisplay,
            ScopedSettingValues {
                user: None,
                guild: None,
            },
        );
        assert_eq!(default_resolved.source, SettingSource::Default);
        assert_eq!(
            default_resolved.value,
            SettingKey::TitleDisplay.default_value()
        );
    }
}
