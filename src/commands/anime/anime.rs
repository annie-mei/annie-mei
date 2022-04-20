use serenity::{
    client::Context,
    framework::standard::{macros::command, Args, CommandResult, Delimiter},
    model::channel::Message,
};
use tokio::task;
use tracing::error;

use super::fetcher::fetcher;

#[command]
async fn anime(ctx: &Context, msg: &Message) -> CommandResult {
    let args = Args::new(&msg.content, &[Delimiter::Single(' ')]);
    let response = task::spawn_blocking(|| fetcher(args)).await?;

    let title = response.description;

    if let Err(why) = msg.channel_id.say(&ctx.http, title).await {
        error!("Error sending message: {:?}", why);
    }

    // let msg = msg
    //     .channel_id
    //     .send_message(&ctx.http, |m| {
    //         m.content("Hello, World!")
    //             .embed(|e| {
    //                 e.colour(0x00ff00)
    //                     .title("This is a title")
    //                     .description("This is a description")
    //                     .image("attachment://mai.jpg")
    //                     .fields(vec![
    //                         ("This is the first field", "This is a field body", true),
    //                         ("This is the second field", "Both fields are inline", true),
    //                     ])
    //                     .field(
    //                         "This is the third field",
    //                         "This is not an inline field",
    //                         false,
    //                     )
    //                     .footer(|f| f.text("This is a footer"))
    //                     .timestamp(chrono::Utc::now())
    //                     .thumbnail("attachment://mai.jpg")
    //             })
    //             .add_file("./mai.jpg")
    //     })
    //     .await;

    // if let Err(why) = msg {
    //     error!("Error sending message: {:?}", why);
    // }

    Ok(())
}
