use serenity::{
    client::Context,
    framework::standard::{macros::command, CommandResult},
    model::channel::Message,
};

use tracing::error;

#[command]
async fn help(ctx: &Context, msg: &Message) -> CommandResult {
    let msg = msg
        .channel_id
        .send_message(&ctx.http, |m| {
            m.embed(|e| {
                e.colour(0x00ff00)
                    .title("Hello there!")
                    .description("Use these commands to interact with Anilist!")
                    .field(
                        "!anime <anilist id/search term>",
                        "Search for an anime",
                        false,
                    )
                    .field(
                        "!manga <anilist id/search term>",
                        "Search for a manga",
                        false,
                    )
                    .field(
                        "!songs <anilist id/search term>",
                        "Lookup the anime's songs",
                        false,
                    )
                    .field("!help", "Show this message", false)
                    .footer(|f| f.text("Annie Mai"))
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
