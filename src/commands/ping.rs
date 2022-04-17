use serenity::{
    client::Context,
    framework::standard::{macros::command, CommandResult},
    model::channel::Message,
    utils::MessageBuilder,
};
use tracing::error;

#[command]
pub async fn ping(ctx: &Context, msg: &Message) -> CommandResult {
    let response = MessageBuilder::new()
        .push("User ")
        .mention(&msg.author.id)
        .push(" used the 'ping' command in the ")
        .push(" channel")
        .build();

    if let Err(why) = msg.channel_id.say(&ctx.http, response).await {
        error!("Error sending message: {:?}", why);
    }

    Ok(())
}
