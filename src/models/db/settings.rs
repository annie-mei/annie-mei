//! SQLx persistence helpers for user and guild settings.

use std::fmt;

use serenity::model::prelude::{GuildId, UserId};
use sqlx::FromRow;
use tracing::instrument;

use crate::{
    models::settings::{
        ResolvedSetting, ScopedSettingValues, SettingKey, SettingValidationError, SettingValue,
        resolve_setting as resolve_scoped_setting,
    },
    utils::{
        database::DbPool,
        privacy::{hash_discord_id, hash_user_id},
    },
};

#[derive(Clone, PartialEq, Eq, FromRow)]
pub struct StoredSettingRow {
    pub setting_key: String,
    pub setting_value: String,
}

impl fmt::Debug for StoredSettingRow {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StoredSettingRow")
            .field("setting_key", &self.setting_key)
            .field("setting_value", &"[REDACTED]")
            .finish()
    }
}

#[derive(Debug)]
pub enum SettingsStorageError {
    Database(sqlx::Error),
    InvalidStoredValue(SettingValidationError),
}

impl fmt::Display for SettingsStorageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Database(error) => write!(f, "database error: {error}"),
            Self::InvalidStoredValue(error) => write!(f, "stored setting is invalid: {error}"),
        }
    }
}

impl std::error::Error for SettingsStorageError {}

impl From<sqlx::Error> for SettingsStorageError {
    fn from(error: sqlx::Error) -> Self {
        Self::Database(error)
    }
}

impl From<SettingValidationError> for SettingsStorageError {
    fn from(error: SettingValidationError) -> Self {
        Self::InvalidStoredValue(error)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResolvedSettingLayers {
    pub user: Option<SettingValue>,
    pub guild: Option<SettingValue>,
    pub effective: ResolvedSetting,
}

#[instrument(
    name = "db.settings.set_user_setting",
    skip(pool, user_discord_id, value),
    fields(
        discord_user_id = %hash_user_id(user_discord_id.get()),
        setting_key = %value.key().as_str()
    )
)]
pub async fn set_user_setting(
    pool: &DbPool,
    user_discord_id: UserId,
    value: SettingValue,
) -> Result<(), SettingsStorageError> {
    sqlx::query(
        "INSERT INTO user_settings (discord_user_id, setting_key, setting_value) \
         VALUES ($1, $2, $3) \
         ON CONFLICT (discord_user_id, setting_key) DO UPDATE \
         SET setting_value = EXCLUDED.setting_value, updated_at = CURRENT_TIMESTAMP",
    )
    .bind(user_discord_id.get().to_string())
    .bind(value.key().as_str())
    .bind(value.as_storage_value())
    .execute(pool)
    .await?;

    Ok(())
}

#[instrument(
    name = "db.settings.set_guild_setting",
    skip(pool, guild_id, value),
    fields(guild_id = %hash_discord_id(guild_id.get()), setting_key = %value.key().as_str())
)]
pub async fn set_guild_setting(
    pool: &DbPool,
    guild_id: GuildId,
    value: SettingValue,
) -> Result<(), SettingsStorageError> {
    sqlx::query(
        "INSERT INTO guild_settings (guild_id, setting_key, setting_value) \
         VALUES ($1, $2, $3) \
         ON CONFLICT (guild_id, setting_key) DO UPDATE \
         SET setting_value = EXCLUDED.setting_value, updated_at = CURRENT_TIMESTAMP",
    )
    .bind(guild_id.get().to_string())
    .bind(value.key().as_str())
    .bind(value.as_storage_value())
    .execute(pool)
    .await?;

    Ok(())
}

#[instrument(
    name = "db.settings.resolve_setting_layers",
    skip(pool, user_discord_id, guild_id),
    fields(
        discord_user_id = %hash_user_id(user_discord_id.get()),
        guild_id = guild_id.map(|id| hash_discord_id(id.get()).to_string()),
        setting_key = %setting_key.as_str()
    )
)]
pub async fn resolve_setting_layers(
    pool: &DbPool,
    user_discord_id: UserId,
    guild_id: Option<GuildId>,
    setting_key: SettingKey,
) -> Result<ResolvedSettingLayers, SettingsStorageError> {
    let mut transaction = pool.begin().await?;

    let user_row = sqlx::query_as::<_, StoredSettingRow>(
        "SELECT setting_key, setting_value FROM user_settings \
         WHERE discord_user_id = $1 AND setting_key = $2",
    )
    .bind(user_discord_id.get().to_string())
    .bind(setting_key.as_str())
    .fetch_optional(&mut *transaction)
    .await?;
    let user = parse_optional_setting(user_row, setting_key)?;

    let guild = match guild_id {
        Some(guild_id) => {
            let row = sqlx::query_as::<_, StoredSettingRow>(
                "SELECT setting_key, setting_value FROM guild_settings \
                 WHERE guild_id = $1 AND setting_key = $2",
            )
            .bind(guild_id.get().to_string())
            .bind(setting_key.as_str())
            .fetch_optional(&mut *transaction)
            .await?;

            parse_optional_setting(row, setting_key)?
        }
        None => None,
    };

    transaction.commit().await?;

    let effective = resolve_scoped_setting(setting_key, ScopedSettingValues { user, guild });

    Ok(ResolvedSettingLayers {
        user,
        guild,
        effective,
    })
}

#[instrument(name = "db.settings.parse_optional_setting", skip(row), fields(setting_key = %setting_key.as_str()))]
fn parse_optional_setting(
    row: Option<StoredSettingRow>,
    setting_key: SettingKey,
) -> Result<Option<SettingValue>, SettingsStorageError> {
    row.map(|row| {
        setting_key
            .parse_value(&row.setting_value)
            .map_err(Into::into)
    })
    .transpose()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stored_setting_debug_redacts_value() {
        let row = StoredSettingRow {
            setting_key: "analytics_privacy".to_string(),
            setting_value: "opted_out".to_string(),
        };

        let debug = format!("{row:?}");

        assert!(debug.contains("analytics_privacy"));
        assert!(debug.contains("[REDACTED]"));
        assert!(!debug.contains("opted_out"));
    }

    #[test]
    fn parse_optional_setting_validates_stored_value() {
        let row = StoredSettingRow {
            setting_key: "title_display".to_string(),
            setting_value: "invalid".to_string(),
        };

        let error = parse_optional_setting(Some(row), SettingKey::TitleDisplay)
            .expect_err("invalid stored value should fail");

        assert!(matches!(error, SettingsStorageError::InvalidStoredValue(_)));
    }
}
