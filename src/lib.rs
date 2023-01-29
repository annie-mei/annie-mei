mod commands;
mod models;
mod utils;

use shuttle_secrets::SecretStore;
use tracing::{debug, info, instrument};

use serenity::{
    async_trait,
    client::{Client, Context, EventHandler},
    framework::standard::{macros::hook, CommandResult, DispatchError, StandardFramework},
    model::{
        application::{command::Command, interaction::Interaction},
        channel::Message,
        event::ResumedEvent,
        gateway::Ready,
        prelude::Activity,
    },
    prelude::*,
    utils::parse_emoji,
};

use utils::statics::{DISCORD_TOKEN, ENV, SENTRY_DSN};

#[hook]
#[instrument]
async fn before(_: &Context, msg: &Message, command_name: &str) -> bool {
    info!(
        "Got command '{}' by user '{}'",
        command_name, msg.author.name
    );
    true
}

#[hook]
#[instrument]
async fn after(_: &Context, _msg: &Message, command_name: &str, command_result: CommandResult) {
    match command_result {
        Ok(()) => info!("Processed command '{}'", command_name),
        Err(why) => info!("Command '{}' returned error {:?}", command_name, why),
    }
}

// TODO: Add default reaction
#[hook]
#[instrument]
async fn unknown_command(ctx: &Context, msg: &Message, unknown_command_name: &str) {
    info!("Could not find command named '{}'", unknown_command_name);
    let reaction = parse_emoji("<:wtf:953730408158228570>").unwrap();
    let _ = msg.react(ctx, reaction).await;
}
// TODO: Figure out how to use this
#[hook]
async fn delay_action(ctx: &Context, msg: &Message) {
    // You may want to handle a Discord rate limit if this fails.
    let _ = msg.react(ctx, '⏱').await;
}

#[hook]
async fn dispatch_error(ctx: &Context, msg: &Message, error: DispatchError, _command_name: &str) {
    if let DispatchError::Ratelimited(info) = error {
        // We notify them only once.
        if info.is_first_try {
            let _ = msg
                .channel_id
                .say(
                    &ctx.http,
                    &format!("Try this again in {} seconds.", info.as_secs()),
                )
                .await;
        }
    }
}

struct Handler;

#[async_trait]
impl EventHandler for Handler {
    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        if let Interaction::ApplicationCommand(mut command) = interaction {
            info!("Received command interaction: {:#?}", command);

            match command.data.name.as_str() {
                "ping" => commands::ping::run(&ctx, &command).await,
                "help" => commands::help::run(&ctx, &command).await,
                "songs" => commands::songs::command::run(&ctx, &mut command).await,
                "manga" => commands::manga::command::run(&ctx, &mut command).await,
                "anime" => commands::anime::command::run(&ctx, &mut command).await,
                _ => {
                    let msg = command
                        .channel_id
                        .send_message(&ctx.http, |msg| {
                            msg.embed(|e| e.title("Error").description("Not implemented"))
                        })
                        .await;
                    if let Err(why) = msg {
                        println!("Error sending message: {:?}", why);
                        info!("Cannot respond to slash command: {}", why);
                    }
                }
            };
        }
    }

    async fn ready(&self, ctx: Context, ready: Ready) {
        let guild_commands = Command::set_global_application_commands(&ctx.http, |commands| {
            commands
                .create_application_command(|command| commands::ping::register(command))
                .create_application_command(|command| commands::help::register(command))
                .create_application_command(|command| commands::songs::command::register(command))
                .create_application_command(|command| commands::manga::command::register(command))
                .create_application_command(|command| commands::anime::command::register(command))
        })
        .await;

        ctx.set_activity(Activity::listening("/help")).await;

        info!(
            "I created the following global slash command: {:#?}",
            guild_commands
        );
        info!("{} is connected!", ready.user.name);
    }

    #[instrument(skip(self, _ctx))]
    async fn resume(&self, _ctx: Context, resume: ResumedEvent) {
        debug!("Resumed; trace: {:?}", resume.trace);
    }
}

#[shuttle_service::main]
// #[instrument]
async fn serenity(
    #[shuttle_secrets::Secrets] secret_store: SecretStore,
) -> shuttle_service::ShuttleSerenity {
    let environment = secret_store
        .get(ENV)
        .expect("Expected an environment in the environment");
    let sentry_dsn = secret_store
        .get(SENTRY_DSN)
        .expect("Expected a sentry dsn in the environment");

    let _guard = sentry::init((
        sentry_dsn,
        sentry::ClientOptions {
            release: sentry::release_name!(),
            environment: Some(environment.into()),
            ..Default::default()
        },
    ));

    let framework = StandardFramework::new()
        .configure(|c| c.prefix("!"))
        .before(before)
        .after(after)
        .unrecognised_command(unknown_command)
        .on_dispatch_error(dispatch_error);
    let token = secret_store
        .get(DISCORD_TOKEN)
        .expect("Expected a token in the environment");
    let intents = GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::DIRECT_MESSAGES
        | GatewayIntents::MESSAGE_CONTENT;

    let client = Client::builder(&token, intents)
        .event_handler(Handler)
        .framework(framework)
        .await
        .expect("Err creating client");

    Ok(client)
}
