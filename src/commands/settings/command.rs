use crate::{
    models::{
        db::settings::{ResolvedSettingLayers, SettingsStorageError, resolve_all_setting_layers},
        settings::{ALL_SETTING_KEYS, SettingKey, SettingValue},
    },
    utils::{
        database::{DbPool, get_pool_from_context},
        formatter::{bold, code},
        privacy::{configure_sentry_scope, hash_discord_id, hash_user_id},
    },
};

use serenity::{
    all::{
        ButtonStyle, CommandInteraction, ComponentInteraction, CreateActionRow, CreateButton,
        CreateInteractionResponse, CreateInteractionResponseMessage, EditInteractionResponse,
        GuildId, UserId,
    },
    builder::CreateCommand,
    client::Context,
};
use tracing::{error, instrument, warn};

const SETTINGS_COMPONENT_PREFIX: &str = "settings";
const SETTINGS_COMPONENT_ID_PREFIX: &str = "settings:";
const OVERVIEW_COMPONENT: &str = "overview";
const CATEGORY_COMPONENT: &str = "category";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsPanelCategory {
    Overview,
    Setting(SettingKey),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingSummary {
    pub layers: ResolvedSettingLayers,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingsPanel {
    pub category: SettingsPanelCategory,
    pub guild_available: bool,
    pub summaries: Vec<SettingSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SettingsComponentId {
    Overview,
    Category(SettingKey),
}

impl SettingsComponentId {
    fn panel_category(&self) -> SettingsPanelCategory {
        match self {
            Self::Overview => SettingsPanelCategory::Overview,
            Self::Category(key) => SettingsPanelCategory::Setting(*key),
        }
    }
}

pub fn register() -> CreateCommand {
    CreateCommand::new("settings").description("Open your Annie Mei settings panel")
}

#[instrument(name = "command.settings.is_component")]
pub fn is_settings_component(custom_id: &str) -> bool {
    custom_id.starts_with(SETTINGS_COMPONENT_ID_PREFIX)
}

#[instrument(name = "command.settings.overview_custom_id")]
pub fn settings_overview_custom_id() -> String {
    format!("{SETTINGS_COMPONENT_PREFIX}:{OVERVIEW_COMPONENT}")
}

#[instrument(name = "command.settings.category_custom_id", fields(setting_key = %key.as_str()))]
pub fn settings_category_custom_id(key: SettingKey) -> String {
    format!(
        "{SETTINGS_COMPONENT_PREFIX}:{CATEGORY_COMPONENT}:{}",
        key.as_str()
    )
}

#[instrument(name = "command.settings.parse_component_id")]
pub fn parse_settings_component_id(custom_id: &str) -> Option<SettingsComponentId> {
    let parts = custom_id.split(':').collect::<Vec<_>>();

    match parts.as_slice() {
        [SETTINGS_COMPONENT_PREFIX, OVERVIEW_COMPONENT] => Some(SettingsComponentId::Overview),
        [SETTINGS_COMPONENT_PREFIX, CATEGORY_COMPONENT, raw_key] => {
            SettingKey::parse(raw_key).map(SettingsComponentId::Category)
        }
        _ => None,
    }
}

#[instrument(name = "command.settings.plan_panel", skip(summaries))]
pub fn plan_settings_panel(
    category: SettingsPanelCategory,
    guild_available: bool,
    summaries: Vec<SettingSummary>,
) -> SettingsPanel {
    SettingsPanel {
        category,
        guild_available,
        summaries,
    }
}

#[instrument(name = "command.settings.render_panel", skip(panel))]
pub fn render_settings_panel(panel: &SettingsPanel) -> String {
    match panel.category {
        SettingsPanelCategory::Overview => render_overview_panel(panel),
        SettingsPanelCategory::Setting(key) => render_category_panel(panel, key),
    }
}

#[instrument(name = "command.settings.run", skip(ctx, interaction))]
pub async fn run(ctx: &Context, interaction: &mut CommandInteraction) {
    let _ = interaction.defer_ephemeral(&ctx.http).await;

    let user = &interaction.user;
    configure_sentry_scope("Settings", user.id.get(), None);

    let Some(database_pool) = get_pool_from_context(ctx).await else {
        respond_to_command(
            interaction,
            ctx,
            "Database is not initialized. Please try again later.".to_string(),
            Vec::new(),
        )
        .await;
        return;
    };

    let response = match load_settings_panel(
        &database_pool,
        user.id,
        interaction.guild_id,
        SettingsPanelCategory::Overview,
    )
    .await
    {
        Ok(panel) => panel,
        Err(error) => {
            log_settings_storage_error("panel", user.id, interaction.guild_id, &error);
            respond_to_command(
                interaction,
                ctx,
                "I hit an internal error while reading your settings. Please try again later."
                    .to_string(),
                Vec::new(),
            )
            .await;
            return;
        }
    };

    respond_to_command(
        interaction,
        ctx,
        render_settings_panel(&response),
        settings_panel_components(response.category),
    )
    .await;
}

#[instrument(name = "command.settings.handle_component", skip(ctx, interaction))]
pub async fn handle_component(ctx: &Context, interaction: &mut ComponentInteraction) {
    configure_sentry_scope("Settings", interaction.user.id.get(), None);

    let Some(component_id) = parse_settings_component_id(&interaction.data.custom_id) else {
        let builder = CreateInteractionResponse::Message(
            CreateInteractionResponseMessage::new()
                .content("I don't recognize that settings control. Please run `/settings` again.")
                .ephemeral(true),
        );
        let _ = interaction.create_response(&ctx.http, builder).await;
        return;
    };

    if let Err(error) = interaction
        .create_response(&ctx.http, CreateInteractionResponse::Acknowledge)
        .await
    {
        warn!(
            error = %error,
            custom_id = %interaction.data.custom_id,
            "Failed to acknowledge settings component interaction"
        );
        return;
    }

    let Some(database_pool) = get_pool_from_context(ctx).await else {
        respond_to_component(
            interaction,
            ctx,
            "Database is not initialized. Please try again later.".to_string(),
            Vec::new(),
        )
        .await;
        return;
    };

    let category = component_id.panel_category();
    let response = match load_settings_panel(
        &database_pool,
        interaction.user.id,
        interaction.guild_id,
        category,
    )
    .await
    {
        Ok(panel) => panel,
        Err(error) => {
            log_settings_storage_error(
                "component",
                interaction.user.id,
                interaction.guild_id,
                &error,
            );
            respond_to_component(
                interaction,
                ctx,
                "I hit an internal error while reading your settings. Please try again later."
                    .to_string(),
                Vec::new(),
            )
            .await;
            return;
        }
    };

    respond_to_component(
        interaction,
        ctx,
        render_settings_panel(&response),
        settings_panel_components(response.category),
    )
    .await;
}

#[instrument(name = "command.settings.load_panel", skip(pool, user_id, guild_id))]
async fn load_settings_panel(
    pool: &DbPool,
    user_id: UserId,
    guild_id: Option<GuildId>,
    category: SettingsPanelCategory,
) -> Result<SettingsPanel, SettingsStorageError> {
    let summaries = resolve_all_setting_layers(pool, user_id, guild_id, &ALL_SETTING_KEYS)
        .await?
        .into_iter()
        .map(|layers| SettingSummary { layers })
        .collect();

    Ok(plan_settings_panel(category, guild_id.is_some(), summaries))
}

#[instrument(name = "command.settings.render_overview", skip(panel))]
fn render_overview_panel(panel: &SettingsPanel) -> String {
    let mut sections = vec![
        format!("{}", bold("Settings")),
        "Read-only summary of your current Annie Mei preferences. Use the buttons below to inspect each category; changing values will be added in a later update.".to_string(),
    ];

    sections.extend(
        panel
            .summaries
            .iter()
            .map(|summary| format_summary(summary, panel.guild_available, false)),
    );

    sections.join("\n\n")
}

#[instrument(name = "command.settings.render_category", skip(panel))]
fn render_category_panel(panel: &SettingsPanel, key: SettingKey) -> String {
    let Some(summary) = panel
        .summaries
        .iter()
        .find(|summary| summary.layers.effective.key == key)
    else {
        return render_overview_panel(panel);
    };

    format!(
        "{}\n{}\n\n{}",
        bold("Settings"),
        "Read-only category details. Use Overview to return to the full settings summary.",
        format_summary(summary, panel.guild_available, true),
    )
}

#[instrument(name = "command.settings.format_summary", skip(summary))]
fn format_summary(
    summary: &SettingSummary,
    guild_available: bool,
    include_details: bool,
) -> String {
    let key = summary.layers.effective.key;
    let mut lines = vec![
        bold(key.label()),
        format!(
            "Effective: {} ({})",
            format_setting_value(summary.layers.effective.value),
            summary.layers.effective.source
        ),
        format!("User: {}", format_optional_layer(summary.layers.user)),
        format!(
            "Guild: {}",
            format_guild_layer(key, summary.layers.guild, guild_available)
        ),
        format!("Default: {}", format_setting_value(key.default_value())),
    ];

    if include_details {
        lines.push(key.description().to_string());
        lines.push(format!("Allowed values: {}", format_allowed_values(key)));
    }

    lines.join("\n")
}

#[instrument(name = "command.settings.format_optional_layer", skip(value))]
fn format_optional_layer(value: Option<SettingValue>) -> String {
    value.map_or_else(|| "not set".to_string(), format_setting_value)
}

#[instrument(name = "command.settings.format_guild_layer", skip(value))]
fn format_guild_layer(
    key: SettingKey,
    value: Option<SettingValue>,
    guild_available: bool,
) -> String {
    if key == SettingKey::AnalyticsPrivacy {
        return "not applicable".to_string();
    }

    if !guild_available {
        return "not available in DMs".to_string();
    }

    format_optional_layer(value)
}

#[instrument(name = "command.settings.format_value", skip(value))]
fn format_setting_value(value: SettingValue) -> String {
    format!(
        "{} ({})",
        code(value.as_storage_value()),
        value.display_label()
    )
}

#[instrument(name = "command.settings.format_allowed_values")]
fn format_allowed_values(key: SettingKey) -> String {
    key.allowed_values()
        .iter()
        .map(|value| code(value))
        .collect::<Vec<_>>()
        .join(", ")
}

#[instrument(name = "command.settings.components")]
fn settings_panel_components(active: SettingsPanelCategory) -> Vec<CreateActionRow> {
    vec![CreateActionRow::Buttons(vec![
        panel_button(
            SettingsPanelCategory::Overview,
            "Overview",
            settings_overview_custom_id(),
            active,
        ),
        panel_button(
            SettingsPanelCategory::Setting(SettingKey::TitleDisplay),
            "Title display",
            settings_category_custom_id(SettingKey::TitleDisplay),
            active,
        ),
        panel_button(
            SettingsPanelCategory::Setting(SettingKey::AnalyticsPrivacy),
            "Analytics privacy",
            settings_category_custom_id(SettingKey::AnalyticsPrivacy),
            active,
        ),
        panel_button(
            SettingsPanelCategory::Setting(SettingKey::GuildScores),
            "Guild scores",
            settings_category_custom_id(SettingKey::GuildScores),
            active,
        ),
    ])]
}

#[instrument(name = "command.settings.panel_button")]
fn panel_button(
    category: SettingsPanelCategory,
    label: &str,
    custom_id: String,
    active: SettingsPanelCategory,
) -> CreateButton {
    let is_active = category == active;
    let style = if is_active {
        ButtonStyle::Secondary
    } else {
        ButtonStyle::Primary
    };

    CreateButton::new(custom_id)
        .label(label)
        .style(style)
        .disabled(is_active)
}

#[instrument(
    name = "command.settings.respond_command",
    skip(interaction, ctx, components)
)]
async fn respond_to_command(
    interaction: &mut CommandInteraction,
    ctx: &Context,
    content: String,
    components: Vec<CreateActionRow>,
) {
    let builder = EditInteractionResponse::new()
        .content(content)
        .components(components);

    let _ = interaction.edit_response(&ctx.http, builder).await;
}

#[instrument(
    name = "command.settings.respond_component",
    skip(interaction, ctx, components)
)]
async fn respond_to_component(
    interaction: &mut ComponentInteraction,
    ctx: &Context,
    content: String,
    components: Vec<CreateActionRow>,
) {
    let builder = EditInteractionResponse::new()
        .content(content)
        .components(components);

    let _ = interaction.edit_response(&ctx.http, builder).await;
}

#[instrument(
    name = "command.settings.log_storage_error",
    skip(error, user_id, guild_id)
)]
fn log_settings_storage_error(
    operation: &str,
    user_id: UserId,
    guild_id: Option<GuildId>,
    error: &SettingsStorageError,
) {
    error!(
        error = %error,
        operation,
        discord_user_id = %hash_user_id(user_id.get()),
        guild_id = guild_id.map(|id| hash_discord_id(id.get()).to_string()),
        "Settings storage operation failed"
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::settings::{
        AnalyticsPrivacyPreference, GuildScoresPreference, ResolvedSetting, SettingSource,
        TitleDisplayPreference,
    };

    fn layers(
        key: SettingKey,
        user: Option<SettingValue>,
        guild: Option<SettingValue>,
        effective: SettingValue,
        source: SettingSource,
    ) -> ResolvedSettingLayers {
        ResolvedSettingLayers {
            user,
            guild,
            effective: ResolvedSetting {
                key,
                value: effective,
                source,
            },
        }
    }

    fn test_summaries() -> Vec<SettingSummary> {
        vec![
            SettingSummary {
                layers: layers(
                    SettingKey::TitleDisplay,
                    Some(SettingValue::TitleDisplay(TitleDisplayPreference::Romaji)),
                    Some(SettingValue::TitleDisplay(TitleDisplayPreference::English)),
                    SettingValue::TitleDisplay(TitleDisplayPreference::Romaji),
                    SettingSource::User,
                ),
            },
            SettingSummary {
                layers: layers(
                    SettingKey::AnalyticsPrivacy,
                    None,
                    None,
                    SettingValue::AnalyticsPrivacy(AnalyticsPrivacyPreference::Standard),
                    SettingSource::Default,
                ),
            },
            SettingSummary {
                layers: layers(
                    SettingKey::GuildScores,
                    Some(SettingValue::GuildScores(GuildScoresPreference::OptedOut)),
                    Some(SettingValue::GuildScores(GuildScoresPreference::Enabled)),
                    SettingValue::GuildScores(GuildScoresPreference::OptedOut),
                    SettingSource::User,
                ),
            },
        ]
    }

    #[test]
    fn register_has_no_options() {
        let value = serde_json::to_value(register()).expect("command serializes");

        assert_eq!(value["name"], "settings");
        assert!(value["options"].as_array().is_none_or(Vec::is_empty));
    }

    #[test]
    fn parses_stable_component_ids() {
        assert_eq!(
            parse_settings_component_id(&settings_overview_custom_id()),
            Some(SettingsComponentId::Overview)
        );
        assert_eq!(
            parse_settings_component_id(&settings_category_custom_id(SettingKey::GuildScores)),
            Some(SettingsComponentId::Category(SettingKey::GuildScores))
        );
        assert_eq!(
            parse_settings_component_id("settings:category:unknown"),
            None
        );
        assert_eq!(
            parse_settings_component_id("search:category:guild_scores"),
            None
        );
    }

    #[test]
    fn detects_settings_components_without_matching_other_prefixes() {
        assert!(is_settings_component("settings:overview"));
        assert!(!is_settings_component("search:settings:overview"));
        assert!(!is_settings_component("settings-panel:overview"));
    }

    #[test]
    fn overview_renders_current_effective_settings() {
        let panel = plan_settings_panel(SettingsPanelCategory::Overview, true, test_summaries());
        let content = render_settings_panel(&panel);

        assert!(content.contains("**Settings**"));
        assert!(content.contains("**Title display**"));
        assert!(content.contains("Effective: `romaji` (Romaji title) (user override)"));
        assert!(content.contains("User: `romaji` (Romaji title)"));
        assert!(content.contains("Guild: `english` (English title)"));
        assert!(content.contains("**Analytics privacy**"));
        assert!(content.contains("Guild: not applicable"));
        assert!(content.contains("**Guild scores**"));
    }

    #[test]
    fn overview_marks_guild_settings_unavailable_in_dms() {
        let panel = plan_settings_panel(SettingsPanelCategory::Overview, false, test_summaries());
        let content = render_settings_panel(&panel);

        assert!(content.contains("Guild: not available in DMs"));
    }

    #[test]
    fn category_panel_renders_selected_details() {
        let panel = plan_settings_panel(
            SettingsPanelCategory::Setting(SettingKey::GuildScores),
            true,
            test_summaries(),
        );
        let content = render_settings_panel(&panel);

        assert!(content.contains("Read-only category details"));
        assert!(content.contains("**Guild scores**"));
        assert!(content.contains("Allowed values: `enabled`, `disabled`, `opted_out`"));
        assert!(!content.contains("**Title display**"));
    }
}
