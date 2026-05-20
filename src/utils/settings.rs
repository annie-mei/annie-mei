use serenity::{all::UserId, client::Context, model::prelude::GuildId};
use tracing::{instrument, warn};

use crate::{
    models::{
        db::settings::{get_guild_setting, resolve_setting_layers},
        settings::{
            SettingKey, SettingValue, TitleDisplayPreference, guild_scores_enabled,
            user_participates_in_guild_scores,
        },
    },
    utils::database::{DbPool, get_pool_from_context},
};

#[instrument(name = "settings.resolve_title_display", skip(ctx, user_id, guild_id))]
pub async fn resolve_title_display_preference(
    ctx: &Context,
    user_id: UserId,
    guild_id: Option<GuildId>,
) -> TitleDisplayPreference {
    let Some(pool) = get_pool_from_context(ctx).await else {
        warn!("Database pool unavailable; using default title display preference");
        return default_title_display_preference();
    };

    match resolve_setting_layers(&pool, user_id, guild_id, SettingKey::TitleDisplay).await {
        Ok(layers) => match layers.effective.value {
            SettingValue::TitleDisplay(preference) => preference,
            SettingValue::AnalyticsPrivacy(_) | SettingValue::GuildScores(_) => {
                warn!("Unexpected non-title value for title display key; using default");
                default_title_display_preference()
            }
        },
        Err(error) => {
            warn!(error = %error, "Failed to resolve title display preference; using default");
            default_title_display_preference()
        }
    }
}

#[instrument(name = "settings.default_title_display")]
fn default_title_display_preference() -> TitleDisplayPreference {
    match SettingKey::TitleDisplay.default_value() {
        SettingValue::TitleDisplay(preference) => preference,
        SettingValue::AnalyticsPrivacy(_) | SettingValue::GuildScores(_) => {
            TitleDisplayPreference::Matched
        }
    }
}

#[instrument(
    name = "settings.resolve_guild_scores_enabled_with_pool",
    skip(pool, guild_id)
)]
pub async fn resolve_guild_scores_enabled_with_pool(
    pool: &DbPool,
    guild_id: Option<GuildId>,
) -> bool {
    let Some(guild_id) = guild_id else {
        return false;
    };

    match get_guild_setting(pool, guild_id, SettingKey::GuildScores).await {
        Ok(value) => guild_scores_enabled(value),
        Err(error) => {
            warn!(error = %error, "Failed to resolve guild scores setting; disabling for privacy");
            false
        }
    }
}

#[instrument(name = "settings.participates_in_guild_scores", skip(value))]
pub fn participates_in_guild_scores(value: Option<SettingValue>) -> bool {
    user_participates_in_guild_scores(value)
}
