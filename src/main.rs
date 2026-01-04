mod commands;
mod models;
mod schema;
mod utils;

use std::env;

use sentry::integrations::tracing as sentry_tracing;
use tracing::{info, instrument};
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
    statics::{DISCORD_TOKEN, ENV, SENTRY_DSN},
};

struct Handler;

#[async_trait]
impl EventHandler for Handler {
    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        if let Interaction::Command(mut command) = interaction {
            info!("Received command interaction: {:#?}", command);

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
#[instrument]
async fn main() {
    let environment = env::var(ENV).expect("Expected an environment in the environment");
    let sentry_dsn = env::var(SENTRY_DSN).expect("Expected a sentry dsn in the environment");

    let _guard = sentry::init((
        sentry_dsn,
        sentry::ClientOptions {
            release: sentry::release_name!(),
            environment: Some(environment.into()),
            ..Default::default()
        },
    ));

    let subscriber = tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .finish();

    subscriber.with(sentry_tracing::layer()).init();

    let connection = &mut utils::database::establish_connection();
    run_migration(connection);

    let token = env::var(DISCORD_TOKEN).expect("Expected a token in the environment");
    let intents = GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::DIRECT_MESSAGES
        | GatewayIntents::GUILD_PRESENCES
        | GatewayIntents::GUILDS;

    let mut client = Client::builder(&token, intents)
        .event_handler(Handler)
        .await
        .expect("Err creating client");

    if let Err(why) = client.start().await {
        println!("Client error: {why:?}");
    }
}
