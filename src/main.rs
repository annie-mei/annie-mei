mod commands;
mod models;
mod utils;

use std::env;
use std::sync::Arc;

use clap::{Parser, Subcommand};
use sentry::integrations::tracing as sentry_tracing;
use tracing::{Instrument, info, info_span, instrument, warn};
use tracing_subscriber::{EnvFilter, prelude::*, util::SubscriberInitExt};

use serenity::{
    all::{CreateEmbed, CreateMessage},
    async_trait,
    builder::CreateCommand,
    client::{Client, Context, EventHandler},
    gateway::ActivityData,
    model::{application::Command, application::Interaction, gateway::Ready},
    prelude::*,
};

use utils::{
    channel::is_nsfw_channel,
    database::{DatabasePoolKey, create_pool, run_migrations},
    llm::{GeminiClient, GeminiClientKey, configured_model_name},
    oauth::{OAuthContextConfigKey, load_context_config},
    posthog::{CommandTelemetryContext, PostHogClient},
    privacy::{hash_discord_id, hash_user_id, redact_url_credentials},
    settings::resolve_analytics_privacy_preference,
    statics::{DISCORD_TOKEN, ENV, SENTRY_DSN, SENTRY_TRACES_SAMPLE_RATE},
    tls::install_rustls_crypto_provider,
};

/// Annie Mei Discord Bot
#[derive(Parser)]
#[command(name = "annie-mei")]
#[command(version = env!("CARGO_PKG_VERSION"))]
#[command(about = "A Discord bot for anime and manga information", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Compute the hashed user ID for Sentry log lookup
    Hash {
        /// The Discord user ID to hash
        user_id: u64,
    },
}

struct Handler {
    posthog: Option<Arc<PostHogClient>>,
    environment: Option<String>,
}

#[async_trait]
impl EventHandler for Handler {
    #[instrument(name = "discord.interaction_create", skip_all)]
    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        if let Interaction::Command(mut command) = interaction {
            let guild_id = command
                .guild_id
                .map(|guild_id| hash_discord_id(guild_id.get()).to_string());
            let command_span = info_span!(
                "discord.command",
                command_name = %command.data.name,
                user_id = %hash_user_id(command.user.id.get()),
                guild_id = guild_id.as_deref()
            );

            async {
                info!("Received command interaction");
                self.capture_command_hit(&ctx, &command);

                match command.data.name.as_str() {
                    "ping" => commands::ping::run(&ctx, &command).await,
                    "help" => commands::help::run(&ctx, &command).await,
                    "songs" => commands::songs::command::run(&ctx, &mut command).await,
                    "manga" => commands::manga::command::run(&ctx, &mut command).await,
                    "anime" => commands::anime::command::run(&ctx, &mut command).await,
                    "search" => commands::search::command::run(&ctx, &mut command).await,
                    "recommend" => commands::recommend::command::run(&ctx, &mut command).await,
                    "character" => commands::character::command::run(&ctx, &mut command).await,
                    "register" => commands::register::command::run(&ctx, &mut command).await,
                    "unregister" => commands::unregister::run(&ctx, &mut command).await,
                    "whoami" => commands::whoami::run(&ctx, &mut command).await,
                    "settings" => commands::settings::run(&ctx, &mut command).await,
                    _ => {
                        let embed = CreateEmbed::new()
                            .title("Error")
                            .description("Not implemented");
                        let builder = CreateMessage::new().embed(embed);
                        let msg = command.channel_id.send_message(&ctx.http, builder).await;
                        if let Err(why) = msg {
                            println!("Error sending message: {why:?}");
                            info!("Cannot respond to slash command: {why}");
                        }
                    }
                };
            }
            .instrument(command_span)
            .await;
        }
    }

    #[instrument(name = "discord.ready", skip_all)]
    async fn ready(&self, ctx: Context, ready: Ready) {
        let commands: Vec<CreateCommand> = vec![
            commands::ping::register(),
            commands::help::register(),
            commands::songs::command::register(),
            commands::manga::command::register(),
            commands::anime::command::register(),
            commands::search::command::register(),
            commands::recommend::command::register(),
            commands::character::command::register(),
            commands::register::command::register(),
            commands::unregister::register(),
            commands::whoami::register(),
            commands::settings::register(),
        ];

        let guild_commands = Command::set_global_commands(&ctx.http, commands).await;

        ctx.set_activity(Some(ActivityData::listening("/help")));

        info!(
            "I created the following global slash command: {:#?}",
            guild_commands
        );
        info!("{} is connected!", ready.user.name);
    }
}

impl Handler {
    #[instrument(name = "posthog.capture_command_hit", skip(self, ctx, command))]
    fn capture_command_hit(&self, ctx: &Context, command: &serenity::all::CommandInteraction) {
        let Some(posthog) = self.posthog.clone() else {
            return;
        };

        let ctx = ctx.clone();
        let user_id = command.user.id;
        let raw_guild_id = command.guild_id;
        let command_name = command.data.name.clone();
        let environment = self.environment.clone();
        let is_dm = command.guild_id.is_none();
        let channel_guild_id = command.guild_id;
        let channel_id = command.channel_id;

        tokio::spawn(
            async move {
                let analytics_privacy = resolve_analytics_privacy_preference(&ctx, user_id).await;
                let analytics_opted_out = analytics_privacy.opted_out();
                let distinct_id =
                    (!analytics_opted_out).then(|| hash_user_id(user_id.get()).to_string());
                let guild_id = (!analytics_opted_out)
                    .then(|| {
                        raw_guild_id.map(|guild_id| hash_discord_id(guild_id.get()).to_string())
                    })
                    .flatten();

                let event = posthog.build_command_hit_event(&CommandTelemetryContext {
                    distinct_id,
                    guild_id,
                    command: command_name,
                    environment,
                    is_dm,
                    channel_nsfw: is_nsfw_channel(&ctx, channel_id, channel_guild_id).await,
                });

                if let Err(error) = posthog.capture(event).await {
                    warn!(error = %error, "PostHog command hit capture failed");
                }
            }
            .instrument(tracing::Span::current()),
        );
    }
}

#[tokio::main]
#[instrument(name = "app.main")]
async fn main() {
    let cli = Cli::parse();

    // Handle CLI subcommands
    if let Some(Commands::Hash { user_id }) = cli.command {
        let hashed = hash_user_id(user_id);
        println!("{}", hashed);
        return;
    }

    // Default: run the bot
    install_rustls_crypto_provider();

    let environment = env::var(ENV).expect("Expected an environment in the environment");
    let telemetry_environment = Some(environment.clone());
    let sentry_dsn = env::var(SENTRY_DSN).expect("Expected a sentry dsn in the environment");
    let sentry_traces_sample_rate_raw = env::var(SENTRY_TRACES_SAMPLE_RATE).ok();
    let (sentry_traces_sample_rate, sentry_traces_sample_rate_invalid) =
        match sentry_traces_sample_rate_raw {
            Some(raw) => match raw.parse::<f32>() {
                Ok(rate) => (rate.clamp(0.0, 1.0), None),
                Err(_) => (0.0, Some(raw)),
            },
            None => (0.0, None),
        };

    let _guard = sentry::init((
        sentry_dsn,
        sentry::ClientOptions {
            release: sentry::release_name!(),
            environment: Some(environment.into()),
            traces_sample_rate: sentry_traces_sample_rate,
            enable_logs: true,
            before_send: Some(Arc::new(|mut event| {
                // Redact URLs with credentials from exception messages
                for exception in event.exception.values.iter_mut() {
                    if let Some(ref mut value) = exception.value {
                        *value = redact_url_credentials(value);
                    }
                }

                // Redact URLs from the event message
                if let Some(ref mut message) = event.message {
                    *message = redact_url_credentials(message);
                }

                // Redact URLs from breadcrumb messages
                for breadcrumb in event.breadcrumbs.values.iter_mut() {
                    if let Some(ref mut message) = breadcrumb.message {
                        *message = redact_url_credentials(message);
                    }
                }

                Some(event)
            })),
            before_send_log: Some(Arc::new(|mut log| {
                log.body = redact_url_credentials(&log.body);
                Some(log)
            })),
            ..Default::default()
        },
    ));

    let env_filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info,serenity=warn"));
    let subscriber = tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .finish();

    subscriber.with(sentry_tracing::layer()).init();

    info!(version = env!("CARGO_PKG_VERSION"), "Annie Mei starting");
    info!(model = %configured_model_name(), "LLM model configured");
    let posthog_client = PostHogClient::from_env().map(Arc::new);
    let gemini_client = match GeminiClient::from_env_with_system_prompt(
        commands::search::prompts::SEARCH_SYSTEM_PROMPT,
    )
    .and_then(|client| client.with_temperature(0.0))
    .map(|client| client.with_posthog(posthog_client.clone()))
    {
        Ok(client) => Some(Arc::new(client)),
        Err(error) => {
            warn!(error = %error, "LLM client unavailable; natural-language search will use fallback search");
            None
        }
    };

    if let Some(invalid_value) = sentry_traces_sample_rate_invalid {
        warn!(
            invalid_value = %invalid_value,
            fallback_sample_rate = sentry_traces_sample_rate,
            "Invalid SENTRY_TRACES_SAMPLE_RATE; defaulting to 0.0"
        );
    }

    if sentry_traces_sample_rate > 0.0 {
        info!(
            sample_rate = sentry_traces_sample_rate,
            "Sentry trace sampling enabled"
        );
    }

    info!("Initializing database connection pool");
    let database_pool = create_pool().await;

    info!("Running database migrations");
    run_migrations(&database_pool)
        .await
        .expect("Failed to run database migrations");

    info!("Loading OAuth configuration");
    let oauth_config = load_context_config().expect("Failed to load OAuth context config");

    let token = env::var(DISCORD_TOKEN).expect("Expected a token in the environment");
    let intents = GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::DIRECT_MESSAGES
        | GatewayIntents::GUILD_PRESENCES
        | GatewayIntents::GUILDS;

    info!("Creating Discord client");
    let mut client = Client::builder(&token, intents)
        .event_handler(Handler {
            posthog: posthog_client,
            environment: telemetry_environment,
        })
        .await
        .expect("Err creating client");

    {
        let mut data = client.data.write().await;
        data.insert::<DatabasePoolKey>(database_pool);
        data.insert::<OAuthContextConfigKey>(Arc::new(oauth_config));
        if let Some(gemini_client) = gemini_client {
            data.insert::<GeminiClientKey>(gemini_client);
        }
    }

    info!("Starting Discord client");
    tokio::select! {
        result = client.start() => {
            if let Err(why) = result {
                tracing::error!(error = %why, "Discord client error");
            }
        }
        _ = tokio::signal::ctrl_c() => {
            info!("Received shutdown signal");
        }
    }

    info!("Shutting down");
    client.shard_manager.shutdown_all().await;
}
