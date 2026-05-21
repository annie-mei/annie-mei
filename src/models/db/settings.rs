//! SQLx persistence helpers for user and guild settings.

use std::{collections::HashMap, fmt};

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

#[derive(Clone, PartialEq, Eq, FromRow)]
struct UserSettingRow {
    discord_user_id: String,
    setting_value: String,
}

impl fmt::Debug for UserSettingRow {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("UserSettingRow")
            .field("discord_user_id", &"[REDACTED]")
            .field("setting_value", &"[REDACTED]")
            .finish()
    }
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
            Self::InvalidStoredValue(error) => {
                write!(
                    f,
                    "stored setting is invalid for {}",
                    error.setting_key.label()
                )
            }
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
#[allow(dead_code)]
pub async fn set_user_setting(
    pool: &DbPool,
    user_discord_id: UserId,
    value: SettingValue,
) -> Result<(), SettingsStorageError> {
    sqlx::query(
        "INSERT INTO annie_mei.user_settings (discord_user_id, setting_key, setting_value) \
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
#[allow(dead_code)]
pub async fn set_guild_setting(
    pool: &DbPool,
    guild_id: GuildId,
    value: SettingValue,
) -> Result<(), SettingsStorageError> {
    sqlx::query(
        "INSERT INTO annie_mei.guild_settings (guild_id, setting_key, setting_value) \
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
    name = "db.settings.get_user_setting",
    skip(pool, user_discord_id),
    fields(
        discord_user_id = %hash_user_id(user_discord_id.get()),
        setting_key = %setting_key.as_str()
    )
)]
pub async fn get_user_setting(
    pool: &DbPool,
    user_discord_id: UserId,
    setting_key: SettingKey,
) -> Result<Option<SettingValue>, SettingsStorageError> {
    let row = sqlx::query_as::<_, StoredSettingRow>(
        "SELECT setting_key, setting_value FROM annie_mei.user_settings \
         WHERE discord_user_id = $1 AND setting_key = $2",
    )
    .bind(user_discord_id.get().to_string())
    .bind(setting_key.as_str())
    .fetch_optional(pool)
    .await?;

    parse_optional_setting(row, setting_key)
}

#[instrument(
    name = "db.settings.get_guild_setting",
    skip(pool, guild_id),
    fields(guild_id = %hash_discord_id(guild_id.get()), setting_key = %setting_key.as_str())
)]
pub async fn get_guild_setting(
    pool: &DbPool,
    guild_id: GuildId,
    setting_key: SettingKey,
) -> Result<Option<SettingValue>, SettingsStorageError> {
    let row = sqlx::query_as::<_, StoredSettingRow>(
        "SELECT setting_key, setting_value FROM annie_mei.guild_settings \
         WHERE guild_id = $1 AND setting_key = $2",
    )
    .bind(guild_id.get().to_string())
    .bind(setting_key.as_str())
    .fetch_optional(pool)
    .await?;

    parse_optional_setting(row, setting_key)
}

#[instrument(
    name = "db.settings.get_user_settings_for_discord_ids",
    skip(pool, user_discord_ids),
    fields(user_count = user_discord_ids.len(), setting_key = %setting_key.as_str())
)]
pub async fn get_user_settings_for_discord_ids(
    pool: &DbPool,
    user_discord_ids: &[String],
    setting_key: SettingKey,
) -> Result<HashMap<String, SettingValue>, SettingsStorageError> {
    if user_discord_ids.is_empty() {
        return Ok(HashMap::new());
    }

    let rows = sqlx::query_as::<_, UserSettingRow>(
        "SELECT discord_user_id, setting_value FROM annie_mei.user_settings \
         WHERE discord_user_id = ANY($1) AND setting_key = $2",
    )
    .bind(user_discord_ids)
    .bind(setting_key.as_str())
    .fetch_all(pool)
    .await?;

    rows.into_iter()
        .map(|row| {
            let value = setting_key.parse_value(&row.setting_value)?;
            Ok((row.discord_user_id, value))
        })
        .collect()
}

#[instrument(
    name = "db.settings.resolve_all_setting_layers",
    skip(pool, user_discord_id, guild_id, setting_keys),
    fields(
        discord_user_id = %hash_user_id(user_discord_id.get()),
        guild_id = guild_id.map(|id| hash_discord_id(id.get()).to_string()),
        setting_count = setting_keys.len()
    )
)]
pub async fn resolve_all_setting_layers(
    pool: &DbPool,
    user_discord_id: UserId,
    guild_id: Option<GuildId>,
    setting_keys: &[SettingKey],
) -> Result<Vec<ResolvedSettingLayers>, SettingsStorageError> {
    if setting_keys.is_empty() {
        return Ok(Vec::new());
    }

    let setting_key_names = setting_keys
        .iter()
        .map(|key| key.as_str().to_string())
        .collect::<Vec<_>>();

    let mut transaction = pool.begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .execute(&mut *transaction)
        .await?;

    let user_rows = sqlx::query_as::<_, StoredSettingRow>(
        "SELECT setting_key, setting_value FROM annie_mei.user_settings \
         WHERE discord_user_id = $1 AND setting_key = ANY($2)",
    )
    .bind(user_discord_id.get().to_string())
    .bind(&setting_key_names)
    .fetch_all(&mut *transaction)
    .await?;
    let user_values = parse_stored_settings(user_rows)?;

    let guild_values = match guild_id {
        Some(guild_id) => {
            let guild_rows = sqlx::query_as::<_, StoredSettingRow>(
                "SELECT setting_key, setting_value FROM annie_mei.guild_settings \
                 WHERE guild_id = $1 AND setting_key = ANY($2)",
            )
            .bind(guild_id.get().to_string())
            .bind(&setting_key_names)
            .fetch_all(&mut *transaction)
            .await?;

            parse_stored_settings(guild_rows)?
        }
        None => Vec::new(),
    };

    transaction.commit().await?;

    Ok(setting_keys
        .iter()
        .map(|key| {
            let user = setting_value_for_key(&user_values, *key);
            let guild = setting_value_for_key(&guild_values, *key);
            let effective = resolve_scoped_setting(*key, ScopedSettingValues { user, guild });

            ResolvedSettingLayers {
                user,
                guild,
                effective,
            }
        })
        .collect())
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
    let layers =
        resolve_all_setting_layers(pool, user_discord_id, guild_id, &[setting_key]).await?;

    Ok(layers
        .into_iter()
        .next()
        .expect("single-key resolve should return one layer"))
}

#[instrument(name = "db.settings.parse_stored_settings", skip(rows))]
fn parse_stored_settings(
    rows: Vec<StoredSettingRow>,
) -> Result<Vec<(SettingKey, SettingValue)>, SettingsStorageError> {
    rows.into_iter()
        .filter_map(|row| {
            SettingKey::parse(&row.setting_key).map(|key| {
                key.parse_value(&row.setting_value)
                    .map(|value| (key, value))
                    .map_err(Into::into)
            })
        })
        .collect()
}

#[instrument(name = "db.settings.setting_value_for_key", skip(values))]
fn setting_value_for_key(
    values: &[(SettingKey, SettingValue)],
    setting_key: SettingKey,
) -> Option<SettingValue> {
    values
        .iter()
        .find_map(|(key, value)| (*key == setting_key).then_some(*value))
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
    fn user_setting_row_debug_redacts_sensitive_fields() {
        let row = UserSettingRow {
            discord_user_id: "123456789".to_string(),
            setting_value: "opted_out".to_string(),
        };

        let debug = format!("{row:?}");

        assert!(debug.contains("UserSettingRow"));
        assert!(debug.contains("[REDACTED]"));
        assert!(!debug.contains("123456789"));
        assert!(!debug.contains("opted_out"));
    }

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

    #[test]
    fn storage_error_display_redacts_invalid_stored_value() {
        let error = SettingsStorageError::InvalidStoredValue(SettingValidationError {
            setting_key: SettingKey::TitleDisplay,
            raw_value: "unexpected-secret-value".to_string(),
        });

        let message = error.to_string();

        assert!(message.contains("Title display"));
        assert!(!message.contains("unexpected-secret-value"));
    }
}
