use crate::{
    models::{anilist_anime::Anime, media_type::MediaType as Type},
    utils::{message::NOT_FOUND_MANGA, response_fetcher::fetcher},
};
use serenity::{
    builder::CreateEmbed,
    client::Context,
    framework::standard::{macros::command, Args, CommandResult, Delimiter},
    model::channel::Message,
};
use tokio::task;
use tracing::error;

#[command]
async fn manga(ctx: &Context, msg: &Message) -> CommandResult {
    let args = Args::new(&msg.content, &[Delimiter::Single(' ')]);
    let response = task::spawn_blocking(|| fetcher(Type::Manga, args)).await?;

    let msg = match response {
        None => {
            msg.channel_id
                .send_message(&ctx.http, |m| m.content(NOT_FOUND_MANGA))
                .await
        }
        Some(anime) => {
            msg.channel_id
                .send_message(&ctx.http, |m| {
                    m.embed(|e| build_message_from_anime(anime.manga(), e))
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
fn build_message_from_anime(anime: Anime, embed: &mut CreateEmbed) -> &mut CreateEmbed {
    embed
        .colour(anime.transform_color())
        .title(anime.transform_romaji_title())
        .description(anime.transform_description_and_mal_link())
        .fields(vec![
            ("Type", "Anime", true),                     // Field 0
            ("Status", &anime.transform_status(), true), // Field 1
            ("Season", &anime.transform_season(), true), // Field 2
        ])
        .fields(vec![
            ("Format", &anime.transform_format(), true), // Field 3
            ("Episodes", &anime.transform_episodes(), true), // Field 4
            ("Duration", &anime.transform_duration(), true), // Field 5
        ])
        .fields(vec![
            ("Source", &anime.transform_source(), true), // Field 6
            ("Average Score", &anime.transform_score(), true), // Field 7
            // ("\u{200b}", &"\u{200b}".to_string(), true), // Would add a blank field
            ("Top Tag", &anime.transform_tags(), true), // Field 8
        ])
        .field("Genres", &anime.transform_genres(), false) // Field 9
        .field("Studios", &anime.transform_studios(), false) // Field 10
        .fields(vec![
            ("Streaming", &anime.transform_links(), true), // Field 11
            ("Trailer", &anime.transform_trailer(), true), // Field 12
        ])
        .footer(|f| f.text(anime.transform_english_title()))
        .url(&anime.transform_anilist())
        .thumbnail(anime.transform_thumbnail())
}
