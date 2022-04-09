use std::env;

use dotenv::dotenv;
use log::{debug, error, info, log_enabled, Level};
use serenity::{
    async_trait,
    client::{Client, Context, EventHandler},
    framework::standard::{
        macros::{command, group},
        CommandResult, StandardFramework,
    },
    model::{channel::Message, gateway::Ready},
};

const HELP_MESSAGE: &str = "
          Hello there, Human!

          You have summoned me. Let's see about getting you what you need.

          ? Need technical help?
          => Post in the <#CHANNEL_ID> channel and other humans will assist you.
          
          ? Looking for the Code of Conduct?
          => Here it is: <https://opensource.facebook.com/code-of-conduct> 
          
          ? Something wrong?
          => You can flag an admin with @admin
          
          I hope that resolves your issue!
          -- Helpbot
          
          ";

// const HELP_COMMAND: &str = "!help";

#[group]
#[commands(help)]
struct General;

struct Handler;

#[async_trait]
impl EventHandler for Handler {
    // async fn message(&self, ctx: Context, msg: Message) {
    //     if msg.content == HELP_COMMAND {
    //         if let Err(why) = msg.channel_id.say(&ctx.http, HELP_MESSAGE).await {
    //             println!("Error sending message: {:?}", why);
    //         }
    //     }
    // }

    async fn ready(&self, _: Context, ready: Ready) {
        println!("{} is connected!", ready.user.name);
    }
}

#[tokio::main]
async fn main() {
    dotenv().ok();

    let framework = StandardFramework::new()
        .configure(|c| c.prefix("!")) // set the bot's prefix to "~"
        .group(&GENERAL_GROUP);
    let token = env::var("DISCORD_TOKEN").expect("Expected a token in the environment");

    let mut client = Client::builder(&token)
        .event_handler(Handler)
        .framework(framework)
        .await
        .expect("Err creating client");

    if let Err(why) = client.start().await {
        println!("Client error: {:?}", why);
    }
}

#[command]
async fn help(ctx: &Context, msg: &Message) -> CommandResult {
    msg.reply(ctx, HELP_MESSAGE).await?;

    Ok(())
}
