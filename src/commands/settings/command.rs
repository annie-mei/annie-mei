use crate::{
    commands::response::CommandResponse,
    models::{
        db::settings::{
            ResolvedSettingLayers, SettingsStorageError, resolve_setting_layers, set_guild_setting,
            set_user_setting,
        },
        settings::{ALL_SETTING_KEYS, SettingKey, SettingScope, SettingValue},
    },
    utils::{
        database::get_pool_from_context,
        privacy::{configure_sentry_scope, hash_discord_id, hash_user_id},
    },
};

use serenity::{
    all::{
        CommandDataOption, CommandDataOptionValue, CommandInteraction, CreateCommandOption,
        EditInteractionResponse, GuildId, Permissions, UserId,
    },
    builder::CreateCommand,
    client::Context,
    model::application::CommandOptionType,
};
use tracing::{error, instrument};

const ACTION_OPTION: &str = "action";
const KEY_OPTION: &str = "key";
const SCOPE_OPTION: &str = "scope";
const VALUE_OPTION: &str = "value";

const ACTION_GET: &str = "get";
const ACTION_SET: &str = "set";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SettingsAction {
    Get,
    Set,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingsCommandOptions {
    action: SettingsAction,
    key: SettingKey,
    scope: SettingScope,
    value: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SettingsContext {
    pub user_id: UserId,
    pub guild_id: Option<GuildId>,
    pub member_permissions: Option<Permissions>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsWriteTarget {
    User(UserId),
    Guild(GuildId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SettingsWriteRequest {
    pub target: SettingsWriteTarget,
    pub value: SettingValue,
}

#[derive(Debug)]
pub enum SettingsCommandPlan {
    Read {
        key: SettingKey,
        scope: SettingScope,
    },
    Write(SettingsWriteRequest),
    Respond(CommandResponse),
}

pub fn register() -> CreateCommand {
    CreateCommand::new("settings")
        .description("View or update Annie Mei preferences")
        .add_option(
            CreateCommandOption::new(
                CommandOptionType::String,
                ACTION_OPTION,
                "View or update a setting",
            )
            .add_string_choice("Get", ACTION_GET)
            .add_string_choice("Set", ACTION_SET)
            .required(true),
        )
        .add_option(
            CreateCommandOption::new(
                CommandOptionType::String,
                KEY_OPTION,
                "The setting to view or update",
            )
            .add_string_choice("Title display", SettingKey::TitleDisplay.as_str())
            .add_string_choice("Guild scores", SettingKey::GuildScores.as_str())
            .add_string_choice("Analytics privacy", SettingKey::AnalyticsPrivacy.as_str())
            .required(true),
        )
        .add_option(
            CreateCommandOption::new(
                CommandOptionType::String,
                SCOPE_OPTION,
                "Which setting layer to use",
            )
            .add_string_choice("Effective", SettingScope::Effective.as_str())
            .add_string_choice("User", SettingScope::User.as_str())
            .add_string_choice("Guild", SettingScope::Guild.as_str())
            .required(true),
        )
        .add_option(
            CreateCommandOption::new(
                CommandOptionType::String,
                VALUE_OPTION,
                "The new value when action is Set",
            )
            .add_string_choice("matched", "matched")
            .add_string_choice("romaji", "romaji")
            .add_string_choice("english", "english")
            .add_string_choice("native", "native")
            .add_string_choice("visible", "visible")
            .add_string_choice("hidden", "hidden")
            .add_string_choice("standard", "standard")
            .add_string_choice("opted_out", "opted_out")
            .required(false),
        )
}

#[instrument(name = "command.settings.parse_options", skip(options))]
pub fn parse_settings_options(
    options: &[CommandDataOption],
) -> Result<SettingsCommandOptions, String> {
    let action = optional_string_option(options, ACTION_OPTION)
        .and_then(parse_action)
        .ok_or_else(|| "Choose whether to `get` or `set` a setting.".to_string())?;

    let key = optional_string_option(options, KEY_OPTION)
        .and_then(SettingKey::parse)
        .ok_or_else(|| settings_help_message("Choose a valid setting key."))?;

    let scope = optional_string_option(options, SCOPE_OPTION)
        .and_then(SettingScope::parse)
        .ok_or_else(|| "Choose a valid scope: `effective`, `user`, or `guild`.".to_string())?;

    let value = optional_string_option(options, VALUE_OPTION).map(ToOwned::to_owned);

    Ok(SettingsCommandOptions {
        action,
        key,
        scope,
        value,
    })
}

#[instrument(name = "command.settings.plan", skip(options, context))]
pub fn plan_settings_command(
    options: SettingsCommandOptions,
    context: SettingsContext,
) -> SettingsCommandPlan {
    match options.action {
        SettingsAction::Get => SettingsCommandPlan::Read {
            key: options.key,
            scope: options.scope,
        },
        SettingsAction::Set => plan_settings_write(options, context),
    }
}

#[instrument(name = "command.settings.format_layers", skip(layers, guild_id))]
pub fn format_setting_layers(
    layers: ResolvedSettingLayers,
    guild_id: Option<GuildId>,
    scope: SettingScope,
) -> String {
    let key = layers.effective.key;
    let selected_layer = match scope {
        SettingScope::Effective => format!(
            "Effective: `{}` ({})",
            layers.effective.value.as_storage_value(),
            layers.effective.source
        ),
        SettingScope::User => format_layer_value("User", layers.user),
        SettingScope::Guild if guild_id.is_some() => format_layer_value("Guild", layers.guild),
        SettingScope::Guild => "Guild: not available in DMs".to_string(),
    };

    format!(
        "**{}**\n{}\n\n{}\nAllowed values: `{}`",
        key.label(),
        selected_layer,
        key.description(),
        key.allowed_values().join("`, `"),
    )
}

#[instrument(name = "command.settings.format_saved", skip(target, value))]
pub fn format_saved_setting(target: SettingsWriteTarget, value: SettingValue) -> String {
    let scope = match target {
        SettingsWriteTarget::User(_) => "user",
        SettingsWriteTarget::Guild(_) => "guild",
    };

    format!(
        "Saved {} for the {} scope as `{}` ({}).",
        value.key().label(),
        scope,
        value.as_storage_value(),
        value.display_label(),
    )
}

#[instrument(name = "command.settings.run", skip(ctx, interaction))]
pub async fn run(ctx: &Context, interaction: &mut CommandInteraction) {
    let _ = interaction.defer_ephemeral(&ctx.http).await;

    let user = &interaction.user;
    configure_sentry_scope("Settings", user.id.get(), None);

    let options = match parse_settings_options(&interaction.data.options) {
        Ok(options) => options,
        Err(message) => {
            respond(interaction, ctx, CommandResponse::Content(message)).await;
            return;
        }
    };

    let context = SettingsContext {
        user_id: user.id,
        guild_id: interaction.guild_id,
        member_permissions: interaction
            .member
            .as_ref()
            .and_then(|member| member.permissions),
    };

    let plan = match plan_settings_command(options, context) {
        SettingsCommandPlan::Respond(response) => {
            respond(interaction, ctx, response).await;
            return;
        }
        plan => plan,
    };

    let Some(database_pool) = get_pool_from_context(ctx).await else {
        respond(
            interaction,
            ctx,
            CommandResponse::Content(
                "Database is not initialized. Please try again later.".to_string(),
            ),
        )
        .await;
        return;
    };

    let response = match plan {
        SettingsCommandPlan::Respond(response) => response,
        SettingsCommandPlan::Read { key, scope } => {
            match resolve_setting_layers(&database_pool, user.id, interaction.guild_id, key).await {
                Ok(layers) => CommandResponse::Content(format_setting_layers(
                    layers,
                    interaction.guild_id,
                    scope,
                )),
                Err(error) => {
                    log_settings_storage_error("read", user.id, interaction.guild_id, &error);
                    CommandResponse::Content(
                        "I hit an internal error while reading that setting. Please try again later."
                            .to_string(),
                    )
                }
            }
        }
        SettingsCommandPlan::Write(request) => match request.target {
            SettingsWriteTarget::User(user_id) => {
                match set_user_setting(&database_pool, user_id, request.value).await {
                    Ok(()) => CommandResponse::Content(format_saved_setting(
                        request.target,
                        request.value,
                    )),
                    Err(error) => {
                        error!(
                            error = %error,
                            discord_user_id = %hash_user_id(user.id.get()),
                            setting_key = %request.value.key().as_str(),
                            "Failed to save user setting"
                        );
                        CommandResponse::Content(
                            "I hit an internal error while saving that setting. Please try again later."
                                .to_string(),
                        )
                    }
                }
            }
            SettingsWriteTarget::Guild(guild_id) => {
                match set_guild_setting(&database_pool, guild_id, request.value).await {
                    Ok(()) => CommandResponse::Content(format_saved_setting(
                        request.target,
                        request.value,
                    )),
                    Err(error) => {
                        error!(
                            error = %error,
                            guild_id = %hash_discord_id(guild_id.get()),
                            setting_key = %request.value.key().as_str(),
                            "Failed to save guild setting"
                        );
                        CommandResponse::Content(
                            "I hit an internal error while saving that setting. Please try again later."
                                .to_string(),
                        )
                    }
                }
            }
        },
    };

    respond(interaction, ctx, response).await;
}

#[instrument(name = "command.settings.plan_write", skip(options, context))]
fn plan_settings_write(
    options: SettingsCommandOptions,
    context: SettingsContext,
) -> SettingsCommandPlan {
    let Some(raw_value) = options.value.as_deref() else {
        return SettingsCommandPlan::Respond(CommandResponse::Content(format!(
            "Provide a `value` when setting {}. Allowed values: `{}`.",
            options.key.label(),
            options.key.allowed_values().join("`, `"),
        )));
    };

    let value = match options.key.parse_value(raw_value) {
        Ok(value) => value,
        Err(error) => {
            return SettingsCommandPlan::Respond(CommandResponse::Content(error.to_string()));
        }
    };

    match options.scope {
        SettingScope::Effective => SettingsCommandPlan::Respond(CommandResponse::Content(
            "`effective` is read-only because it is resolved from user, guild, and default settings. Choose `user` or `guild` when setting a value."
                .to_string(),
        )),
        SettingScope::User => SettingsCommandPlan::Write(SettingsWriteRequest {
            target: SettingsWriteTarget::User(context.user_id),
            value,
        }),
        SettingScope::Guild => {
            let Some(guild_id) = context.guild_id else {
                return SettingsCommandPlan::Respond(CommandResponse::Content(
                    "Guild settings can only be changed from inside a server.".to_string(),
                ));
            };

            if !can_manage_guild_settings(context.member_permissions) {
                return SettingsCommandPlan::Respond(CommandResponse::Content(
                    "You need the Manage Server permission to change guild settings.".to_string(),
                ));
            }

            SettingsCommandPlan::Write(SettingsWriteRequest {
                target: SettingsWriteTarget::Guild(guild_id),
                value,
            })
        }
    }
}

#[instrument(name = "command.settings.optional_string_option", skip(options))]
fn optional_string_option<'a>(options: &'a [CommandDataOption], name: &str) -> Option<&'a str> {
    options
        .iter()
        .find(|option| option.name == name)
        .and_then(|option| match &option.value {
            CommandDataOptionValue::String(value) => Some(value.as_str()),
            _ => None,
        })
}

#[instrument(name = "command.settings.parse_action")]
fn parse_action(raw: &str) -> Option<SettingsAction> {
    match raw {
        ACTION_GET => Some(SettingsAction::Get),
        ACTION_SET => Some(SettingsAction::Set),
        _ => None,
    }
}

#[instrument(name = "command.settings.can_manage_guild_settings")]
fn can_manage_guild_settings(permissions: Option<Permissions>) -> bool {
    permissions.is_some_and(|permissions| {
        permissions.contains(Permissions::MANAGE_GUILD)
            || permissions.contains(Permissions::ADMINISTRATOR)
    })
}

#[instrument(name = "command.settings.format_layer_value", skip(value))]
fn format_layer_value(label: &str, value: Option<SettingValue>) -> String {
    match value {
        Some(value) => format!(
            "{label}: `{}` ({})",
            value.as_storage_value(),
            value.display_label()
        ),
        None => format!("{label}: not set"),
    }
}

#[instrument(name = "command.settings.help_message")]
fn settings_help_message(prefix: &str) -> String {
    let settings = ALL_SETTING_KEYS
        .iter()
        .map(|key| format!("`{}`", key.as_str()))
        .collect::<Vec<_>>()
        .join(", ");

    format!("{prefix} Available settings: {settings}.")
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

#[instrument(name = "command.settings.respond", skip(interaction, ctx, response))]
async fn respond(interaction: &mut CommandInteraction, ctx: &Context, response: CommandResponse) {
    let builder = match response {
        CommandResponse::Content(content) => EditInteractionResponse::new().content(content),
        CommandResponse::Message(content) => EditInteractionResponse::new().content(content),
        CommandResponse::Embed(embed) => EditInteractionResponse::new().embed(*embed),
    };

    let _ = interaction.edit_response(&ctx.http, builder).await;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn string_options(pairs: &[(&str, &str)]) -> Vec<CommandDataOption> {
        let values = pairs
            .iter()
            .map(|(name, value)| {
                serde_json::json!({
                    "name": name,
                    "type": 3,
                    "value": value,
                })
            })
            .collect::<Vec<_>>();

        serde_json::from_value(serde_json::Value::Array(values)).expect("options deserialize")
    }

    #[test]
    fn parses_get_options() {
        let options = string_options(&[
            (ACTION_OPTION, ACTION_GET),
            (KEY_OPTION, "title_display"),
            (SCOPE_OPTION, "effective"),
        ]);

        let parsed = parse_settings_options(&options).expect("options should parse");

        assert_eq!(parsed.action, SettingsAction::Get);
        assert_eq!(parsed.key, SettingKey::TitleDisplay);
        assert_eq!(parsed.scope, SettingScope::Effective);
        assert_eq!(parsed.value, None);
    }

    #[test]
    fn plans_read_with_requested_scope() {
        let options = SettingsCommandOptions {
            action: SettingsAction::Get,
            key: SettingKey::TitleDisplay,
            scope: SettingScope::User,
            value: None,
        };
        let context = SettingsContext {
            user_id: UserId::new(42),
            guild_id: Some(GuildId::new(7)),
            member_permissions: None,
        };

        let SettingsCommandPlan::Read { key, scope } = plan_settings_command(options, context)
        else {
            panic!("expected read request")
        };

        assert_eq!(key, SettingKey::TitleDisplay);
        assert_eq!(scope, SettingScope::User);
    }

    #[test]
    fn formats_only_requested_read_scope() {
        let layers = ResolvedSettingLayers {
            user: SettingKey::TitleDisplay.parse_value("romaji").ok(),
            guild: SettingKey::TitleDisplay.parse_value("english").ok(),
            effective: crate::models::settings::resolve_setting(
                SettingKey::TitleDisplay,
                crate::models::settings::ScopedSettingValues {
                    user: SettingKey::TitleDisplay.parse_value("romaji").ok(),
                    guild: SettingKey::TitleDisplay.parse_value("english").ok(),
                },
            ),
        };

        let content = format_setting_layers(layers, Some(GuildId::new(7)), SettingScope::User);

        assert!(content.contains("User: `romaji`"));
        assert!(!content.contains("Guild: `english`"));
        assert!(!content.contains("Effective: `romaji`"));
    }

    #[test]
    fn plans_user_write_with_valid_value() {
        let options = SettingsCommandOptions {
            action: SettingsAction::Set,
            key: SettingKey::GuildScores,
            scope: SettingScope::User,
            value: Some("hidden".to_string()),
        };
        let context = SettingsContext {
            user_id: UserId::new(42),
            guild_id: None,
            member_permissions: None,
        };

        let SettingsCommandPlan::Write(request) = plan_settings_command(options, context) else {
            panic!("expected write request")
        };

        assert_eq!(request.target, SettingsWriteTarget::User(UserId::new(42)));
        assert_eq!(
            request.value,
            SettingKey::GuildScores.parse_value("hidden").unwrap()
        );
    }

    #[test]
    fn rejects_guild_write_in_dm() {
        let options = SettingsCommandOptions {
            action: SettingsAction::Set,
            key: SettingKey::TitleDisplay,
            scope: SettingScope::Guild,
            value: Some("romaji".to_string()),
        };
        let context = SettingsContext {
            user_id: UserId::new(42),
            guild_id: None,
            member_permissions: None,
        };

        let SettingsCommandPlan::Respond(response) = plan_settings_command(options, context) else {
            panic!("expected response")
        };

        assert!(response.unwrap_content().contains("inside a server"));
    }

    #[test]
    fn rejects_guild_write_without_manage_guild_permission() {
        let options = SettingsCommandOptions {
            action: SettingsAction::Set,
            key: SettingKey::TitleDisplay,
            scope: SettingScope::Guild,
            value: Some("romaji".to_string()),
        };
        let context = SettingsContext {
            user_id: UserId::new(42),
            guild_id: Some(GuildId::new(7)),
            member_permissions: Some(Permissions::VIEW_CHANNEL),
        };

        let SettingsCommandPlan::Respond(response) = plan_settings_command(options, context) else {
            panic!("expected response")
        };

        assert!(response.unwrap_content().contains("Manage Server"));
    }

    #[test]
    fn allows_guild_write_with_manage_guild_permission() {
        let options = SettingsCommandOptions {
            action: SettingsAction::Set,
            key: SettingKey::AnalyticsPrivacy,
            scope: SettingScope::Guild,
            value: Some("opted_out".to_string()),
        };
        let context = SettingsContext {
            user_id: UserId::new(42),
            guild_id: Some(GuildId::new(7)),
            member_permissions: Some(Permissions::MANAGE_GUILD),
        };

        let SettingsCommandPlan::Write(request) = plan_settings_command(options, context) else {
            panic!("expected write request")
        };

        assert_eq!(request.target, SettingsWriteTarget::Guild(GuildId::new(7)));
        assert_eq!(
            request.value,
            SettingKey::AnalyticsPrivacy
                .parse_value("opted_out")
                .unwrap()
        );
    }

    #[test]
    fn allows_guild_write_with_administrator_permission() {
        let options = SettingsCommandOptions {
            action: SettingsAction::Set,
            key: SettingKey::AnalyticsPrivacy,
            scope: SettingScope::Guild,
            value: Some("opted_out".to_string()),
        };
        let context = SettingsContext {
            user_id: UserId::new(42),
            guild_id: Some(GuildId::new(7)),
            member_permissions: Some(Permissions::ADMINISTRATOR),
        };

        let SettingsCommandPlan::Write(request) = plan_settings_command(options, context) else {
            panic!("expected write request")
        };

        assert_eq!(request.target, SettingsWriteTarget::Guild(GuildId::new(7)));
    }

    #[test]
    fn rejects_writing_effective_scope() {
        let options = SettingsCommandOptions {
            action: SettingsAction::Set,
            key: SettingKey::TitleDisplay,
            scope: SettingScope::Effective,
            value: Some("english".to_string()),
        };
        let context = SettingsContext {
            user_id: UserId::new(42),
            guild_id: Some(GuildId::new(7)),
            member_permissions: Some(Permissions::MANAGE_GUILD),
        };

        let SettingsCommandPlan::Respond(response) = plan_settings_command(options, context) else {
            panic!("expected response")
        };

        assert!(response.unwrap_content().contains("read-only"));
    }

    #[test]
    fn invalid_value_returns_validation_message() {
        let options = SettingsCommandOptions {
            action: SettingsAction::Set,
            key: SettingKey::TitleDisplay,
            scope: SettingScope::User,
            value: Some("kana".to_string()),
        };
        let context = SettingsContext {
            user_id: UserId::new(42),
            guild_id: None,
            member_permissions: None,
        };

        let SettingsCommandPlan::Respond(response) = plan_settings_command(options, context) else {
            panic!("expected response")
        };

        let content = response.unwrap_content();
        assert!(content.contains("kana"));
        assert!(content.contains("matched, romaji, english, native"));
    }
}
