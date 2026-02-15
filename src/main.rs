mod commands;
mod models;
mod schema;
mod utils;

use std::env;
use std::sync::Arc;

use clap::{Parser, Subcommand};
use sentry::integrations::tracing as sentry_tracing;
use tracing::{info, info_span, instrument};
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
    database::run_migration,
    privacy::{hash_user_id, redact_url_credentials},
    statics::{DISCORD_TOKEN, ENV, SENTRY_DSN, SENTRY_TRACES_SAMPLE_RATE},
};

/// Annie Mei Discord Bot
#[derive(Parser)]
#[command(name = "annie-mei")]
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

struct Handler;

#[async_trait]
impl EventHandler for Handler {
    #[instrument(name = "discord.interaction_create", skip_all)]
    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        if let Interaction::Command(mut command) = interaction {
            let command_span = info_span!(
                "discord.command",
                command_name = %command.data.name,
                user_id = command.user.id.get(),
                guild_id = ?command.guild_id
            );
            let _command_span = command_span.enter();

            info!("Received command interaction");

            match command.data.name.as_str() {
                "ping" => commands::ping::run(&ctx, &command).await,
                "help" => commands::help::run(&ctx, &command).await,
                "songs" => commands::songs::command::run(&ctx, &mut command).await,
                "manga" => commands::manga::command::run(&ctx, &mut command).await,
                "anime" => commands::anime::command::run(&ctx, &mut command).await,
                "register" => commands::register::command::run(&ctx, &mut command).await,
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
    }

    #[instrument(name = "discord.ready", skip_all)]
    async fn ready(&self, ctx: Context, ready: Ready) {
        let commands: Vec<CreateCommand> = vec![
            commands::ping::register(),
            commands::help::register(),
            commands::songs::command::register(),
            commands::manga::command::register(),
            commands::anime::command::register(),
            commands::register::command::register(),
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
    let environment = env::var(ENV).expect("Expected an environment in the environment");
    let sentry_dsn = env::var(SENTRY_DSN).expect("Expected a sentry dsn in the environment");
    let sentry_traces_sample_rate = env::var(SENTRY_TRACES_SAMPLE_RATE)
        .ok()
        .and_then(|raw| raw.parse::<f32>().ok())
        .map(|rate| rate.clamp(0.0, 1.0))
        .unwrap_or(0.0);

    if sentry_traces_sample_rate > 0.0 {
        info!(
            sample_rate = sentry_traces_sample_rate,
            "Sentry trace sampling enabled"
        );
    }

    let _guard = sentry::init((
        sentry_dsn,
        sentry::ClientOptions {
            release: sentry::release_name!(),
            environment: Some(environment.into()),
            traces_sample_rate: sentry_traces_sample_rate,
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
            ..Default::default()
        },
    ));

    let env_filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info,serenity=warn"));
    let subscriber = tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .finish();

    subscriber.with(sentry_tracing::layer()).init();

    info!("Initializing database connection");
    let connection = &mut utils::database::establish_connection();
    run_migration(connection);

    let token = env::var(DISCORD_TOKEN).expect("Expected a token in the environment");
    let intents = GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::DIRECT_MESSAGES
        | GatewayIntents::GUILD_PRESENCES
        | GatewayIntents::GUILDS;

    info!("Creating Discord client");
    let mut client = Client::builder(&token, intents)
        .event_handler(Handler)
        .await
        .expect("Err creating client");

    info!("Starting Discord client");
    if let Err(why) = client.start().await {
        println!("Client error: {why:?}");
    }
}
