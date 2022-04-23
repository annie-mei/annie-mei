use super::{fetcher::fetcher, model::Anime};

use serenity::{
    builder::CreateEmbed,
    client::Context,
    framework::standard::{macros::command, Args, CommandResult, Delimiter},
    model::channel::Message,
};
use tokio::task;
use tracing::error;

#[command]
async fn anime(ctx: &Context, msg: &Message) -> CommandResult {
    let args = Args::new(&msg.content, &[Delimiter::Single(' ')]);
    let response = task::spawn_blocking(|| fetcher(args)).await?;

    let msg = msg
        .channel_id
        .send_message(&ctx.http, |m| {
            m.embed(|e| build_message_from_anime(response, e))
        })
        .await;

    if let Err(why) = msg {
        error!("Error sending message: {:?}", why);
    }

    Ok(())
}

// TODO: Maybe use https://docs.rs/serenity/latest/serenity/model/channel/struct.Message.html
// and send proper embeds
fn build_message_from_anime(anime: Anime, embed: &mut CreateEmbed) -> &mut CreateEmbed {
    embed
        .colour(anime.transform_color())
        .title(anime.transform_title())
        .description(anime.transform_description())
        // .image(anime.cover_image.large.to_string())
        .fields(vec![
            ("Type", "Anime", true),
            ("Status", &anime.transform_status(), true),
            ("Season", &anime.transform_season(), true),
        ])
        .fields(vec![
            ("Format", &anime.transform_format(), true),
            ("Episodes", &anime.transform_episodes(), true),
            ("Duration", &anime.transform_duration(), true),
        ])
        .fields(vec![
            ("Source", &anime.transform_source(), true),
            ("Average Score", &anime.transform_score(), true),
        ])
        .field("Genres", &anime.transform_genres(), false)
        .field("Studios", &anime.transform_studios(), false)
        .fields(vec![
            ("Streaming", &anime.transform_links(), true),
            ("Trailer", &anime.transform_trailer(), true),
        ])
        .footer(|f| f.text(anime.transform_mal_id()))
        .url(&anime.transform_anilist())
        .thumbnail(anime.transform_thumbnail())
}
