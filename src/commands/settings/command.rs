use crate::{
    commands::response::CommandResponse,
    models::{
        db::settings::{
            ResolvedSettingLayers, SettingsStorageError, get_guild_setting, get_user_setting,
            resolve_setting_layers, set_guild_setting, set_user_setting,
        },
        settings::{ALL_SETTING_KEYS, ALL_SETTING_SCOPES, SettingKey, SettingScope, SettingValue},
    },
    utils::{
        database::{DbPool, get_pool_from_context},
        formatter::{bold, code},
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SettingsReadRequest {
    pub user_id: UserId,
    pub guild_id: Option<GuildId>,
    pub key: SettingKey,
    pub scope: SettingScope,
}

#[derive(Debug)]
pub enum SettingsCommandPlan {
    Read(SettingsReadRequest),
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
        .add_option(setting_key_option())
        .add_option(setting_scope_option())
        .add_option(setting_value_option())
}

#[instrument(name = "command.settings.key_option")]
fn setting_key_option() -> CreateCommandOption {
    ALL_SETTING_KEYS
        .iter()
        .fold(
            CreateCommandOption::new(
                CommandOptionType::String,
                KEY_OPTION,
                "The setting to view or update",
            ),
            |option, key| option.add_string_choice(key.label(), key.as_str()),
        )
        .required(true)
}

#[instrument(name = "command.settings.scope_option")]
fn setting_scope_option() -> CreateCommandOption {
    ALL_SETTING_SCOPES
        .iter()
        .fold(
            CreateCommandOption::new(
                CommandOptionType::String,
                SCOPE_OPTION,
                "Which setting layer to use",
            ),
            |option, scope| option.add_string_choice(scope.label(), scope.as_str()),
        )
        .required(true)
}

#[instrument(name = "command.settings.value_option")]
fn setting_value_option() -> CreateCommandOption {
    let option = CreateCommandOption::new(
        CommandOptionType::String,
        VALUE_OPTION,
        "The new value when action is Set",
    );

    setting_value_choices()
        .into_iter()
        .fold(option, |option, value| {
            option.add_string_choice(value, value)
        })
        .required(false)
}

#[instrument(name = "command.settings.value_choices")]
fn setting_value_choices() -> Vec<&'static str> {
    let mut values = Vec::new();

    for value in ALL_SETTING_KEYS
        .iter()
        .flat_map(|key| key.allowed_values().iter().copied())
    {
        if !values.contains(&value) {
            values.push(value);
        }
    }

    values
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
        SettingsAction::Get => SettingsCommandPlan::Read(SettingsReadRequest {
            user_id: context.user_id,
            guild_id: context.guild_id,
            key: options.key,
            scope: options.scope,
        }),
        SettingsAction::Set => plan_settings_write(options, context),
    }
}

#[instrument(name = "command.settings.format_effective", skip(layers))]
pub fn format_effective_setting(layers: ResolvedSettingLayers) -> String {
    let key = layers.effective.key;
    format_setting_read(
        key,
        format!(
            "Effective: {} ({})",
            code(layers.effective.value.as_storage_value()),
            layers.effective.source
        ),
    )
}

#[instrument(name = "command.settings.format_scoped", skip(value))]
pub fn format_scoped_setting(key: SettingKey, label: &str, value: Option<SettingValue>) -> String {
    format_setting_read(key, format_layer_value(label, value))
}

#[instrument(name = "command.settings.format_read")]
fn format_setting_read(key: SettingKey, selected_layer: String) -> String {
    format!(
        "{}\n{}\n\n{}\nAllowed values: {}",
        bold(key.label()),
        selected_layer,
        key.description(),
        format_allowed_values(key),
    )
}

#[instrument(name = "command.settings.format_saved", skip(target, value))]
pub fn format_saved_setting(target: SettingsWriteTarget, value: SettingValue) -> String {
    let scope = match target {
        SettingsWriteTarget::User(_) => "user",
        SettingsWriteTarget::Guild(_) => "guild",
    };

    format!(
        "Saved {} for the {} scope as {} ({}).",
        value.key().label(),
        scope,
        code(value.as_storage_value()),
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
        SettingsCommandPlan::Read(request) => {
            match read_settings_response(&database_pool, request).await {
                Ok(response) => response,
                Err(error) => {
                    log_settings_storage_error("read", request.user_id, request.guild_id, &error);
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

#[instrument(name = "command.settings.read_response", skip(pool, request))]
async fn read_settings_response(
    pool: &DbPool,
    request: SettingsReadRequest,
) -> Result<CommandResponse, SettingsStorageError> {
    let content = match request.scope {
        SettingScope::Effective => {
            let layers =
                resolve_setting_layers(pool, request.user_id, request.guild_id, request.key)
                    .await?;
            format_effective_setting(layers)
        }
        SettingScope::User => {
            let value = get_user_setting(pool, request.user_id, request.key).await?;
            format_scoped_setting(request.key, "User", value)
        }
        SettingScope::Guild => {
            let Some(guild_id) = request.guild_id else {
                return Ok(CommandResponse::Content(
                    "Guild settings are not available in DMs.".to_string(),
                ));
            };

            let value = get_guild_setting(pool, guild_id, request.key).await?;
            format_scoped_setting(request.key, "Guild", value)
        }
    };

    Ok(CommandResponse::Content(content))
}

#[instrument(name = "command.settings.plan_write", skip(options, context))]
fn plan_settings_write(
    options: SettingsCommandOptions,
    context: SettingsContext,
) -> SettingsCommandPlan {
    let Some(raw_value) = options.value.as_deref() else {
        return SettingsCommandPlan::Respond(CommandResponse::Content(format!(
            "Provide a `value` when setting {}. Allowed values: {}.",
            options.key.label(),
            format_allowed_values(options.key),
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
            "{label}: {} ({})",
            code(value.as_storage_value()),
            value.display_label()
        ),
        None => format!("{label}: not set"),
    }
}

#[instrument(name = "command.settings.format_allowed_values")]
fn format_allowed_values(key: SettingKey) -> String {
    key.allowed_values()
        .iter()
        .map(|value| code(value))
        .collect::<Vec<_>>()
        .join(", ")
}

#[instrument(name = "command.settings.help_message")]
fn settings_help_message(prefix: &str) -> String {
    let settings = ALL_SETTING_KEYS
        .iter()
        .map(|key| code(key.as_str()))
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

        let SettingsCommandPlan::Read(request) = plan_settings_command(options, context) else {
            panic!("expected read request")
        };

        assert_eq!(request.user_id, UserId::new(42));
        assert_eq!(request.guild_id, Some(GuildId::new(7)));
        assert_eq!(request.key, SettingKey::TitleDisplay);
        assert_eq!(request.scope, SettingScope::User);
    }

    #[test]
    fn formats_only_requested_read_scope() {
        let content = format_scoped_setting(
            SettingKey::TitleDisplay,
            "User",
            SettingKey::TitleDisplay.parse_value("romaji").ok(),
        );

        assert!(content.contains("User: `romaji`"));
        assert!(!content.contains("Guild: `english`"));
        assert!(!content.contains("Effective: `romaji`"));
    }

    #[test]
    fn plans_user_write_with_valid_value() {
        let options = SettingsCommandOptions {
            action: SettingsAction::Set,
            key: SettingKey::AnalyticsPrivacy,
            scope: SettingScope::User,
            value: Some("opted_out".to_string()),
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
            SettingKey::AnalyticsPrivacy
                .parse_value("opted_out")
                .unwrap()
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
