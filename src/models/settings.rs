//! Typed settings registry shared by commands and feature code.

use std::fmt;

use tracing::instrument;

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
    AnalyticsPrivacy,
    GuildScores,
}

pub const ALL_SETTING_KEYS: [SettingKey; 3] = [
    SettingKey::TitleDisplay,
    SettingKey::AnalyticsPrivacy,
    SettingKey::GuildScores,
];

impl SettingKey {
    pub fn parse(raw: &str) -> Option<Self> {
        match normalize_token(raw).as_str() {
            "title_display" | "title" | "titles" => Some(Self::TitleDisplay),
            "analytics_privacy" | "analytics" | "privacy" => Some(Self::AnalyticsPrivacy),
            "guild_scores" | "guild_score" | "scores" => Some(Self::GuildScores),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::TitleDisplay => "title_display",
            Self::AnalyticsPrivacy => "analytics_privacy",
            Self::GuildScores => "guild_scores",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::TitleDisplay => "Title display",
            Self::AnalyticsPrivacy => "Analytics privacy",
            Self::GuildScores => "Guild scores",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Self::TitleDisplay => "Which AniList title variant Annie Mei should prefer.",
            Self::AnalyticsPrivacy => {
                "Whether user-level analytics may include raw user-provided content. The default is `standard`; `opted_out` disables raw query, prompt, and output capture in supported analytics/observability while keeping pseudonymous operational telemetry."
            }
            Self::GuildScores => {
                "Whether guild score displays are enabled for a server and whether a user participates. Guild disabled wins over user participation; users who opt out are excluded."
            }
        }
    }

    pub fn default_value(self) -> SettingValue {
        match self {
            Self::TitleDisplay => SettingValue::TitleDisplay(TitleDisplayPreference::Matched),
            Self::AnalyticsPrivacy => {
                SettingValue::AnalyticsPrivacy(AnalyticsPrivacyPreference::Standard)
            }
            Self::GuildScores => SettingValue::GuildScores(GuildScoresPreference::Enabled),
        }
    }

    pub fn allowed_values(self) -> &'static [&'static str] {
        match self {
            Self::TitleDisplay => &["matched", "romaji", "english", "native"],
            Self::AnalyticsPrivacy => &["standard", "opted_out"],
            Self::GuildScores => &["enabled", "disabled", "opted_out"],
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
            Self::AnalyticsPrivacy => match normalized.as_str() {
                "standard" | "enabled" | "on" | "default" => {
                    SettingValue::AnalyticsPrivacy(AnalyticsPrivacyPreference::Standard)
                }
                "opted_out" | "optout" | "private" | "disabled" | "off" => {
                    SettingValue::AnalyticsPrivacy(AnalyticsPrivacyPreference::OptedOut)
                }
                _ => return Err(SettingValidationError::new(self, raw)),
            },
            Self::GuildScores => match normalized.as_str() {
                "enabled" | "enable" | "on" | "default" | "participating" => {
                    SettingValue::GuildScores(GuildScoresPreference::Enabled)
                }
                "disabled" | "disable" | "off" => {
                    SettingValue::GuildScores(GuildScoresPreference::Disabled)
                }
                "opted_out" | "optout" | "private" | "excluded" => {
                    SettingValue::GuildScores(GuildScoresPreference::OptedOut)
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
pub enum AnalyticsPrivacyPreference {
    Standard,
    OptedOut,
}

impl AnalyticsPrivacyPreference {
    pub fn opted_out(self) -> bool {
        matches!(self, Self::OptedOut)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuildScoresPreference {
    Enabled,
    Disabled,
    OptedOut,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingValue {
    TitleDisplay(TitleDisplayPreference),
    AnalyticsPrivacy(AnalyticsPrivacyPreference),
    GuildScores(GuildScoresPreference),
}

impl SettingValue {
    pub fn key(self) -> SettingKey {
        match self {
            Self::TitleDisplay(_) => SettingKey::TitleDisplay,
            Self::AnalyticsPrivacy(_) => SettingKey::AnalyticsPrivacy,
            Self::GuildScores(_) => SettingKey::GuildScores,
        }
    }

    pub fn as_storage_value(self) -> &'static str {
        match self {
            Self::TitleDisplay(TitleDisplayPreference::Matched) => "matched",
            Self::TitleDisplay(TitleDisplayPreference::Romaji) => "romaji",
            Self::TitleDisplay(TitleDisplayPreference::English) => "english",
            Self::TitleDisplay(TitleDisplayPreference::Native) => "native",
            Self::AnalyticsPrivacy(AnalyticsPrivacyPreference::Standard) => "standard",
            Self::AnalyticsPrivacy(AnalyticsPrivacyPreference::OptedOut) => "opted_out",
            Self::GuildScores(GuildScoresPreference::Enabled) => "enabled",
            Self::GuildScores(GuildScoresPreference::Disabled) => "disabled",
            Self::GuildScores(GuildScoresPreference::OptedOut) => "opted_out",
        }
    }

    pub fn display_label(self) -> &'static str {
        match self {
            Self::TitleDisplay(TitleDisplayPreference::Matched) => "matched title",
            Self::TitleDisplay(TitleDisplayPreference::Romaji) => "Romaji title",
            Self::TitleDisplay(TitleDisplayPreference::English) => "English title",
            Self::TitleDisplay(TitleDisplayPreference::Native) => "native title",
            Self::AnalyticsPrivacy(AnalyticsPrivacyPreference::Standard) => "standard analytics",
            Self::AnalyticsPrivacy(AnalyticsPrivacyPreference::OptedOut) => {
                "opted out of analytics"
            }
            Self::GuildScores(GuildScoresPreference::Enabled) => "guild scores enabled",
            Self::GuildScores(GuildScoresPreference::Disabled) => "guild scores disabled",
            Self::GuildScores(GuildScoresPreference::OptedOut) => "opted out of guild scores",
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
    if key == SettingKey::GuildScores {
        return resolve_guild_scores_setting(values);
    }

    if key == SettingKey::AnalyticsPrivacy {
        return resolve_user_only_setting(key, values.user);
    }

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

#[instrument(name = "settings.resolve_user_only", skip(user))]
fn resolve_user_only_setting(key: SettingKey, user: Option<SettingValue>) -> ResolvedSetting {
    if let Some(value) = user {
        return ResolvedSetting {
            key,
            value,
            source: SettingSource::User,
        };
    }

    ResolvedSetting {
        key,
        value: key.default_value(),
        source: SettingSource::Default,
    }
}

#[instrument(name = "settings.resolve_guild_scores", skip(values))]
fn resolve_guild_scores_setting(values: ScopedSettingValues) -> ResolvedSetting {
    if matches!(
        values.guild,
        Some(SettingValue::GuildScores(GuildScoresPreference::Disabled))
    ) {
        return ResolvedSetting {
            key: SettingKey::GuildScores,
            value: SettingValue::GuildScores(GuildScoresPreference::Disabled),
            source: SettingSource::Guild,
        };
    }

    if matches!(
        values.user,
        Some(SettingValue::GuildScores(GuildScoresPreference::OptedOut))
    ) {
        return ResolvedSetting {
            key: SettingKey::GuildScores,
            value: SettingValue::GuildScores(GuildScoresPreference::OptedOut),
            source: SettingSource::User,
        };
    }

    if let Some(value @ SettingValue::GuildScores(_)) = values.user {
        return ResolvedSetting {
            key: SettingKey::GuildScores,
            value,
            source: SettingSource::User,
        };
    }

    if let Some(value @ SettingValue::GuildScores(_)) = values.guild {
        return ResolvedSetting {
            key: SettingKey::GuildScores,
            value,
            source: SettingSource::Guild,
        };
    }

    ResolvedSetting {
        key: SettingKey::GuildScores,
        value: SettingKey::GuildScores.default_value(),
        source: SettingSource::Default,
    }
}

#[instrument(name = "settings.guild_scores_enabled", skip(guild_value))]
pub fn guild_scores_enabled(guild_value: Option<SettingValue>) -> bool {
    match guild_value {
        Some(SettingValue::GuildScores(GuildScoresPreference::Disabled)) => false,
        Some(SettingValue::GuildScores(GuildScoresPreference::OptedOut)) => false,
        Some(SettingValue::GuildScores(GuildScoresPreference::Enabled)) | None => true,
        Some(_) => true,
    }
}

#[instrument(name = "settings.user_participates_in_guild_scores", skip(user_value))]
pub fn user_participates_in_guild_scores(user_value: Option<SettingValue>) -> bool {
    match user_value {
        Some(SettingValue::GuildScores(GuildScoresPreference::OptedOut)) => false,
        Some(SettingValue::GuildScores(GuildScoresPreference::Disabled)) => false,
        Some(SettingValue::GuildScores(GuildScoresPreference::Enabled)) | None => true,
        Some(_) => true,
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

    #[test]
    fn resolve_analytics_privacy_uses_user_value_or_explicit_default() {
        let guild = SettingKey::AnalyticsPrivacy.parse_value("opted_out").ok();
        let user = SettingKey::AnalyticsPrivacy.parse_value("standard").ok();

        let user_resolved = resolve_setting(
            SettingKey::AnalyticsPrivacy,
            ScopedSettingValues { user, guild },
        );
        assert_eq!(user_resolved.source, SettingSource::User);
        assert_eq!(
            user_resolved.value,
            SettingValue::AnalyticsPrivacy(AnalyticsPrivacyPreference::Standard)
        );

        let default_resolved = resolve_setting(
            SettingKey::AnalyticsPrivacy,
            ScopedSettingValues { user: None, guild },
        );
        assert_eq!(default_resolved.source, SettingSource::Default);
        assert_eq!(
            default_resolved.value,
            SettingValue::AnalyticsPrivacy(AnalyticsPrivacyPreference::Standard)
        );
    }

    #[test]
    fn analytics_privacy_opted_out_helper_identifies_opt_out() {
        assert!(!AnalyticsPrivacyPreference::Standard.opted_out());
        assert!(AnalyticsPrivacyPreference::OptedOut.opted_out());
    }

    #[test]
    fn resolve_setting_uses_guild_scores_precedence() {
        let resolved = resolve_setting(
            SettingKey::GuildScores,
            ScopedSettingValues {
                user: Some(SettingValue::GuildScores(GuildScoresPreference::Enabled)),
                guild: Some(SettingValue::GuildScores(GuildScoresPreference::Disabled)),
            },
        );

        assert_eq!(resolved.source, SettingSource::Guild);
        assert_eq!(
            resolved.value,
            SettingValue::GuildScores(GuildScoresPreference::Disabled)
        );

        let resolved = resolve_setting(
            SettingKey::GuildScores,
            ScopedSettingValues {
                user: Some(SettingValue::GuildScores(GuildScoresPreference::OptedOut)),
                guild: Some(SettingValue::GuildScores(GuildScoresPreference::Enabled)),
            },
        );

        assert_eq!(resolved.source, SettingSource::User);
        assert_eq!(
            resolved.value,
            SettingValue::GuildScores(GuildScoresPreference::OptedOut)
        );
    }

    #[test]
    fn guild_scores_default_enabled_with_guild_disable_precedence() {
        assert!(guild_scores_enabled(None));
        assert!(guild_scores_enabled(Some(SettingValue::GuildScores(
            GuildScoresPreference::Enabled
        ))));
        assert!(!guild_scores_enabled(Some(SettingValue::GuildScores(
            GuildScoresPreference::Disabled
        ))));
        assert!(!guild_scores_enabled(Some(SettingValue::GuildScores(
            GuildScoresPreference::OptedOut
        ))));
    }

    #[test]
    fn guild_scores_user_opt_out_excludes_participant() {
        assert!(user_participates_in_guild_scores(None));
        assert!(user_participates_in_guild_scores(Some(
            SettingValue::GuildScores(GuildScoresPreference::Enabled)
        )));
        assert!(!user_participates_in_guild_scores(Some(
            SettingValue::GuildScores(GuildScoresPreference::OptedOut)
        )));
        assert!(!user_participates_in_guild_scores(Some(
            SettingValue::GuildScores(GuildScoresPreference::Disabled)
        )));
    }
}
