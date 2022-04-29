use super::fetcher::fetcher;
use crate::models::anime::Anime;
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

    let msg = match response {
        None => {
            msg.channel_id
                .send_message(&ctx.http, |m| m.content("No anime with that name found :("))
                .await
        }
        Some(anime) => {
            msg.channel_id
                .send_message(&ctx.http, |m| {
                    m.embed(|e| build_message_from_anime(anime, e))
                })
                .await
        }
    };

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
        .title(anime.transform_romaji_title())
        .description(anime.transform_description())
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
            // ("\u{200b}", &"\u{200b}".to_string(), true),
            ("Top Tag", &anime.transform_tags(), true),
        ])
        .field("Genres", &anime.transform_genres(), false)
        .field("Studios", &anime.transform_studios(), false)
        .fields(vec![
            ("Streaming", &anime.transform_links(), true),
            ("Trailer", &anime.transform_trailer(), true),
        ])
        .footer(|f| f.text(anime.transform_english_title()))
        .url(&anime.transform_anilist())
        .thumbnail(anime.transform_thumbnail())
}
