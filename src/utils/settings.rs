use serenity::{all::UserId, client::Context, model::prelude::GuildId};
use tracing::{instrument, warn};

use crate::{
    models::{
        db::settings::resolve_setting_layers,
        settings::{SettingKey, SettingValue, TitleDisplayPreference},
    },
    utils::database::get_pool_from_context,
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
            SettingValue::AnalyticsPrivacy(_) => default_title_display_preference(),
        },
        Err(error) => {
            warn!(error = %error, "Failed to resolve title display preference; using default");
            default_title_display_preference()
        }
    }
}

fn default_title_display_preference() -> TitleDisplayPreference {
    match SettingKey::TitleDisplay.default_value() {
        SettingValue::TitleDisplay(preference) => preference,
        SettingValue::AnalyticsPrivacy(_) => TitleDisplayPreference::Matched,
    }
}
