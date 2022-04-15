use serenity::{
    client::Context,
    framework::standard::{
        macros::command,
        CommandResult,
    },
    model::channel::Message,
    // utils::MessageBuilder,
};
use tracing::error;

const HELP_MESSAGE: &str = "
          Hello there, Human!          
          ";

#[command]
async fn help(ctx: &Context, msg: &Message) -> CommandResult {
    if let Err(why) = msg.channel_id.say(&ctx.http, HELP_MESSAGE).await {
        error!("Error sending message: {:?}", why);
    }

    Ok(())
}