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


#[command]
pub async fn ping(ctx: &Context, msg: &Message) -> CommandResult {
    if let Err(why) = msg.channel_id.say(&ctx.http, "Pong! : )").await {
        error!("Error sending message: {:?}", why);
    }

    Ok(())
}