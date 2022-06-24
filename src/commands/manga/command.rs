use crate::{
    models::{anilist_anime::Anime, anilist_manga::Manga, media_type::MediaType as Type},
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
        Some(manga) => {
            msg.channel_id
                .send_message(&ctx.http, |m| {
                    m.embed(|e| build_message_from_manga(manga.manga(), e))
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
fn build_message_from_manga(manga: Manga, embed: &mut CreateEmbed) -> &mut CreateEmbed {
    embed
        .colour(manga.transform_color())
        .title(manga.transform_romaji_title())
        .description(manga.transform_description_and_mal_link())
        .fields(vec![
            ("Type", "Manga", true),                       // Field 0
            ("Status", &manga.transform_status(), true),   // Field 1
            ("Start Date", &manga.transform_date(), true), // Field 2
        ])
        .fields(vec![
            ("Format", &manga.transform_format(), true), // Field 3
            ("Chapters", &manga.transform_chapters(), true), // Field 4
            ("Volumes", &manga.transform_volumes(), true), // Field 5
        ])
        .fields(vec![
            ("Source", &manga.transform_source(), true), // Field 6
            ("Average Score", &manga.transform_score(), true), // Field 7
            // ("\u{200b}", &"\u{200b}".to_string(), true), // Would add a blank field
            ("Top Tag", &manga.transform_tags(), true), // Field 8
        ])
        .field("Genres", &manga.transform_genres(), false) // Field 9
        // .field("Studios", &manga.transform_studios(), false) // Field 10
        // TODO: Change this to a reader
        .fields(vec![
            ("Streaming", &manga.transform_links(), true), // Field 11
        ])
        .footer(|f| f.text(manga.transform_english_title()))
        .url(&manga.transform_anilist())
        .thumbnail(manga.transform_thumbnail())
}
