use crate::{models::mal_response::MalResponse, utils::message::NOT_FOUND_ANIME};

use super::fetcher::fetcher as SongFetcher;
use serenity::{
    builder::CreateEmbed,
    client::Context,
    framework::standard::{macros::command, Args, CommandResult, Delimiter},
    model::channel::Message,
};
use tokio::task;
use tracing::error;

#[command]
async fn songs(ctx: &Context, msg: &Message) -> CommandResult {
    let args = Args::new(&msg.content, &[Delimiter::Single(' ')]);
    let response = task::spawn_blocking(|| SongFetcher(args)).await?;

    let msg = match response {
        None => {
            msg.channel_id
                .send_message(&ctx.http, |m| m.content(NOT_FOUND_ANIME))
                .await
        }
        Some(song_response) => {
            msg.channel_id
                .send_message(&ctx.http, |m| {
                    m.embed(|e| build_message_from_song_response(song_response, e))
                })
                .await
        }
    };

    if let Err(why) = msg {
        error!("Error sending message: {:?}", why);
    }

    Ok(())
}

// TODO: Move this to Utils
// TODO: Maybe use https://docs.rs/serenity/latest/serenity/model/channel/struct.Message.html
//                 https://docs.rs/serenity/latest/serenity/model/channel/struct.Embed.html
// and send proper embeds

fn build_message_from_song_response(
    mal_response: MalResponse,
    embed: &mut CreateEmbed,
) -> &mut CreateEmbed {
    embed
        .title(mal_response.transform_title())
        .field("Openings", mal_response.transform_openings(), false)
        .field("Endings", mal_response.transform_endings(), false)
        .thumbnail(mal_response.transform_thumbnail())
        // TODO: Also Add Anilist Link??
        .field("\u{200b}", mal_response.transform_mal_link(), false)
}
