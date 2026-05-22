use crate::{
    models::{
        db::settings::{
            ResolvedSettingLayers, SettingsStorageError, resolve_all_setting_layers,
            set_guild_setting, set_user_setting,
        },
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
        ButtonStyle, CommandInteraction, ComponentInteraction, ComponentInteractionDataKind,
        CreateActionRow, CreateButton, CreateInteractionResponse, CreateInteractionResponseMessage,
        CreateSelectMenu, CreateSelectMenuKind, CreateSelectMenuOption, EditInteractionResponse,
        GuildId, Permissions, UserId,
    },
    builder::CreateCommand,
    client::Context,
};
use tracing::{error, instrument, warn};

const SETTINGS_COMPONENT_PREFIX: &str = "settings";
const SETTINGS_COMPONENT_ID_PREFIX: &str = "settings:";
const OVERVIEW_COMPONENT: &str = "overview";
const CATEGORY_COMPONENT: &str = "category";
const SET_COMPONENT: &str = "set";

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
    Set(SettingScope, SettingKey),
}

impl SettingsComponentId {
    fn panel_category(&self) -> SettingsPanelCategory {
        match self {
            Self::Overview => SettingsPanelCategory::Overview,
            Self::Category(key) => SettingsPanelCategory::Setting(*key),
            Self::Set(_, key) => SettingsPanelCategory::Setting(*key),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingScope {
    User,
    Guild,
}

impl SettingScope {
    fn parse(raw: &str) -> Option<Self> {
        match raw {
            "user" => Some(Self::User),
            "guild" => Some(Self::Guild),
            _ => None,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Guild => "guild",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::User => "user setting",
            Self::Guild => "server setting",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsWritePlan {
    User(SettingValue),
    Guild(SettingValue),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SettingsWriteError {
    MissingGuild,
    MissingManageGuild,
    InvalidValue(String),
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

#[instrument(name = "command.settings.set_custom_id", fields(scope = %scope.as_str(), setting_key = %key.as_str()))]
pub fn settings_set_custom_id(scope: SettingScope, key: SettingKey) -> String {
    format!(
        "{SETTINGS_COMPONENT_PREFIX}:{SET_COMPONENT}:{}:{}",
        scope.as_str(),
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
        [SETTINGS_COMPONENT_PREFIX, SET_COMPONENT, raw_scope, raw_key] => {
            let scope = SettingScope::parse(raw_scope)?;
            let key = SettingKey::parse(raw_key)?;
            Some(SettingsComponentId::Set(scope, key))
        }
        _ => None,
    }
}

#[instrument(name = "command.settings.plan_write")]
pub fn plan_settings_write(
    scope: SettingScope,
    key: SettingKey,
    raw_value: &str,
    guild_available: bool,
    can_manage_guild: bool,
) -> Result<SettingsWritePlan, SettingsWriteError> {
    let value = key
        .parse_value(raw_value)
        .map_err(|error| SettingsWriteError::InvalidValue(error.to_string()))?;

    if !allowed_values_for_scope(scope, key).contains(&value.as_storage_value()) {
        return Err(SettingsWriteError::InvalidValue(format!(
            "{} cannot be saved as a {}.",
            format_setting_value(value),
            scope.label()
        )));
    }

    match scope {
        SettingScope::User => Ok(SettingsWritePlan::User(value)),
        SettingScope::Guild if !guild_available => Err(SettingsWriteError::MissingGuild),
        SettingScope::Guild if !can_manage_guild => Err(SettingsWriteError::MissingManageGuild),
        SettingScope::Guild => Ok(SettingsWritePlan::Guild(value)),
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

#[instrument(
    name = "command.settings.render_panel_with_notice",
    skip(panel, notice)
)]
fn render_settings_panel_with_notice(panel: &SettingsPanel, notice: Option<&str>) -> String {
    match notice {
        Some(notice) => format!("{}\n\n{}", bold(notice), render_settings_panel(panel)),
        None => render_settings_panel(panel),
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
        settings_panel_components(&response),
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
    let notice = match component_id {
        SettingsComponentId::Overview | SettingsComponentId::Category(_) => None,
        SettingsComponentId::Set(scope, key) => {
            let Some(raw_value) = selected_string_value(interaction) else {
                respond_to_component(
                    interaction,
                    ctx,
                    "I couldn't read that settings value. Please try again.".to_string(),
                    Vec::new(),
                )
                .await;
                return;
            };

            match plan_settings_write(
                scope,
                key,
                raw_value,
                interaction.guild_id.is_some(),
                can_manage_guild_settings(interaction),
            ) {
                Ok(SettingsWritePlan::User(value)) => {
                    if let Err(error) =
                        set_user_setting(&database_pool, interaction.user.id, value).await
                    {
                        log_settings_storage_error(
                            "set_user",
                            interaction.user.id,
                            interaction.guild_id,
                            &error,
                        );
                        respond_to_component(
                            interaction,
                            ctx,
                            "I hit an internal error while saving your setting. Please try again later."
                                .to_string(),
                            Vec::new(),
                        )
                        .await;
                        return;
                    }

                    Some(format!(
                        "Saved {} as your {}.",
                        format_setting_value(value),
                        key.label()
                    ))
                }
                Ok(SettingsWritePlan::Guild(value)) => {
                    let Some(guild_id) = interaction.guild_id else {
                        respond_to_component(
                            interaction,
                            ctx,
                            settings_write_error_message(SettingsWriteError::MissingGuild),
                            Vec::new(),
                        )
                        .await;
                        return;
                    };

                    if let Err(error) = set_guild_setting(&database_pool, guild_id, value).await {
                        log_settings_storage_error(
                            "set_guild",
                            interaction.user.id,
                            interaction.guild_id,
                            &error,
                        );
                        respond_to_component(
                            interaction,
                            ctx,
                            "I hit an internal error while saving the server setting. Please try again later."
                                .to_string(),
                            Vec::new(),
                        )
                        .await;
                        return;
                    }

                    Some(format!(
                        "Saved {} as this server's {}.",
                        format_setting_value(value),
                        key.label()
                    ))
                }
                Err(error) => Some(settings_write_error_message(error)),
            }
        }
    };

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
        render_settings_panel_with_notice(&response, notice.as_deref()),
        settings_panel_components(&response),
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
        "Summary of your current Annie Mei preferences. Use the buttons below to inspect and edit each category.".to_string(),
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
        "Use the menus below to update this category. Use Overview to return to the full settings summary.",
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

#[instrument(name = "command.settings.components", skip(panel))]
fn settings_panel_components(panel: &SettingsPanel) -> Vec<CreateActionRow> {
    let active = panel.category;
    let mut rows = vec![CreateActionRow::Buttons(vec![
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
    ])];

    if let SettingsPanelCategory::Setting(key) = active {
        if let Some(summary) = panel
            .summaries
            .iter()
            .find(|summary| summary.layers.effective.key == key)
        {
            rows.push(setting_select_row(
                SettingScope::User,
                key,
                summary.layers.user,
            ));

            if guild_select_available(key, panel.guild_available) {
                rows.push(setting_select_row(
                    SettingScope::Guild,
                    key,
                    summary.layers.guild,
                ));
            }
        }
    }

    rows
}

#[instrument(name = "command.settings.setting_select_row", skip(selected))]
fn setting_select_row(
    scope: SettingScope,
    key: SettingKey,
    selected: Option<SettingValue>,
) -> CreateActionRow {
    let options = allowed_values_for_scope(scope, key)
        .iter()
        .filter_map(|raw_value| setting_select_option(key, raw_value, selected))
        .collect::<Vec<_>>();

    CreateActionRow::SelectMenu(
        CreateSelectMenu::new(
            settings_set_custom_id(scope, key),
            CreateSelectMenuKind::String { options },
        )
        .placeholder(format!("Set {} {}", scope.as_str(), key.label()))
        .min_values(1)
        .max_values(1),
    )
}

#[instrument(name = "command.settings.setting_select_option", skip(selected))]
fn setting_select_option(
    key: SettingKey,
    raw_value: &str,
    selected: Option<SettingValue>,
) -> Option<CreateSelectMenuOption> {
    let value = key.parse_value(raw_value).ok()?;
    Some(
        CreateSelectMenuOption::new(value.display_label(), value.as_storage_value())
            .description(raw_value)
            .default_selection(selected == Some(value)),
    )
}

#[instrument(name = "command.settings.guild_select_available")]
fn guild_select_available(key: SettingKey, guild_available: bool) -> bool {
    guild_available && key != SettingKey::AnalyticsPrivacy
}

#[instrument(name = "command.settings.allowed_values_for_scope")]
fn allowed_values_for_scope(scope: SettingScope, key: SettingKey) -> &'static [&'static str] {
    match (scope, key) {
        (SettingScope::User, SettingKey::GuildScores) => &["enabled", "opted_out"],
        (SettingScope::Guild, SettingKey::GuildScores) => &["enabled", "disabled"],
        (SettingScope::Guild, SettingKey::AnalyticsPrivacy) => &[],
        _ => key.allowed_values(),
    }
}

#[instrument(name = "command.settings.selected_string_value", skip(interaction))]
fn selected_string_value(interaction: &ComponentInteraction) -> Option<&str> {
    match &interaction.data.kind {
        ComponentInteractionDataKind::StringSelect { values } => values.first().map(String::as_str),
        _ => None,
    }
}

#[instrument(name = "command.settings.can_manage_guild", skip(interaction))]
fn can_manage_guild_settings(interaction: &ComponentInteraction) -> bool {
    interaction
        .member
        .as_ref()
        .and_then(|member| member.permissions)
        .is_some_and(|permissions| permissions.contains(Permissions::MANAGE_GUILD))
}

#[instrument(name = "command.settings.write_error_message")]
fn settings_write_error_message(error: SettingsWriteError) -> String {
    match error {
        SettingsWriteError::MissingGuild => {
            "Server settings can only be changed from inside a server.".to_string()
        }
        SettingsWriteError::MissingManageGuild => {
            "You need the Manage Server permission to change server settings.".to_string()
        }
        SettingsWriteError::InvalidValue(message) => message,
    }
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
            parse_settings_component_id(&settings_set_custom_id(
                SettingScope::Guild,
                SettingKey::TitleDisplay
            )),
            Some(SettingsComponentId::Set(
                SettingScope::Guild,
                SettingKey::TitleDisplay
            ))
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

        assert!(content.contains("Use the menus below to update this category"));
        assert!(content.contains("**Guild scores**"));
        assert!(content.contains("Allowed values: `enabled`, `disabled`, `opted_out`"));
        assert!(!content.contains("**Title display**"));
    }

    #[test]
    fn category_components_include_edit_selects_for_title_user_and_guild_scopes() {
        let panel = plan_settings_panel(
            SettingsPanelCategory::Setting(SettingKey::TitleDisplay),
            true,
            test_summaries(),
        );

        let value = serde_json::to_value(settings_panel_components(&panel))
            .expect("components should serialize");

        assert_eq!(value.as_array().expect("rows").len(), 3);
        assert!(value.to_string().contains(&settings_set_custom_id(
            SettingScope::User,
            SettingKey::TitleDisplay
        )));
        assert!(value.to_string().contains(&settings_set_custom_id(
            SettingScope::Guild,
            SettingKey::TitleDisplay
        )));
    }

    #[test]
    fn analytics_privacy_components_only_include_user_scope() {
        let panel = plan_settings_panel(
            SettingsPanelCategory::Setting(SettingKey::AnalyticsPrivacy),
            true,
            test_summaries(),
        );

        let value = serde_json::to_value(settings_panel_components(&panel))
            .expect("components should serialize");

        assert_eq!(value.as_array().expect("rows").len(), 2);
        assert!(value.to_string().contains(&settings_set_custom_id(
            SettingScope::User,
            SettingKey::AnalyticsPrivacy
        )));
        assert!(
            !value
                .to_string()
                .contains("settings:set:guild:analytics_privacy")
        );
    }

    #[test]
    fn guild_scores_scope_values_prevent_invalid_combinations() {
        assert_eq!(
            allowed_values_for_scope(SettingScope::User, SettingKey::GuildScores),
            &["enabled", "opted_out"]
        );
        assert_eq!(
            allowed_values_for_scope(SettingScope::Guild, SettingKey::GuildScores),
            &["enabled", "disabled"]
        );
    }

    #[test]
    fn plans_user_setting_writes_for_title_privacy_and_guild_score_participation() {
        assert_eq!(
            plan_settings_write(
                SettingScope::User,
                SettingKey::TitleDisplay,
                "native",
                false,
                false,
            ),
            Ok(SettingsWritePlan::User(SettingValue::TitleDisplay(
                TitleDisplayPreference::Native
            )))
        );
        assert_eq!(
            plan_settings_write(
                SettingScope::User,
                SettingKey::AnalyticsPrivacy,
                "opted_out",
                false,
                false,
            ),
            Ok(SettingsWritePlan::User(SettingValue::AnalyticsPrivacy(
                AnalyticsPrivacyPreference::OptedOut
            )))
        );
        assert_eq!(
            plan_settings_write(
                SettingScope::User,
                SettingKey::GuildScores,
                "enabled",
                true,
                false,
            ),
            Ok(SettingsWritePlan::User(SettingValue::GuildScores(
                GuildScoresPreference::Enabled
            )))
        );
    }

    #[test]
    fn guild_setting_writes_require_server_and_manage_server_permission() {
        assert_eq!(
            plan_settings_write(
                SettingScope::Guild,
                SettingKey::TitleDisplay,
                "english",
                false,
                true,
            ),
            Err(SettingsWriteError::MissingGuild)
        );
        assert_eq!(
            plan_settings_write(
                SettingScope::Guild,
                SettingKey::TitleDisplay,
                "english",
                true,
                false,
            ),
            Err(SettingsWriteError::MissingManageGuild)
        );
        assert_eq!(
            plan_settings_write(
                SettingScope::Guild,
                SettingKey::GuildScores,
                "disabled",
                true,
                true,
            ),
            Ok(SettingsWritePlan::Guild(SettingValue::GuildScores(
                GuildScoresPreference::Disabled
            )))
        );
    }

    #[test]
    fn rejects_invalid_scope_value_combinations_with_clear_messages() {
        let user_disabled = plan_settings_write(
            SettingScope::User,
            SettingKey::GuildScores,
            "disabled",
            true,
            false,
        )
        .expect_err("users should not set guild-score disabled");
        let guild_opted_out = plan_settings_write(
            SettingScope::Guild,
            SettingKey::GuildScores,
            "opted_out",
            true,
            true,
        )
        .expect_err("guilds should not set opted_out");

        assert!(settings_write_error_message(user_disabled).contains("user setting"));
        assert!(settings_write_error_message(guild_opted_out).contains("server setting"));
    }
}
