use serenity::{
    client::Context,
    framework::standard::{
        macros::command,
        CommandResult,
    },
    model::channel::Message,
};

use tracing::error;

#[command]
async fn help(ctx: &Context, msg: &Message) -> CommandResult {

    let msg = msg
                .channel_id
                .send_message(&ctx.http, |m| {
                    m.content("Hello, World!")
                        .embed(|e| {
                            e
                                .colour(0x00ff00)
                                .title("This is a title")
                                .description("This is a description")
                                .image("attachment://mai.jpg")
                                .fields(vec![
                                    ("This is the first field", "This is a field body", true),
                                    ("This is the second field", "Both fields are inline", true),
                                ])
                                .field("This is the third field", "This is not an inline field", false)
                                .footer(|f| f.text("This is a footer"))
                                .timestamp(chrono::Utc::now())
                                .thumbnail("attachment://mai.jpg")
                        })
                        .add_file("./mai.jpg")
                })
                .await;

    if let Err(why) = msg {
        error!("Error sending message: {:?}", why);
    }

    Ok(())
}