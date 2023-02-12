use std::collections::HashMap;

use crate::{
    models::{anilist_manga::Manga, media_type::MediaType as Type, transformers::Transformers},
    utils::{
        guild::{get_current_guild_members, get_guild_scores_for_media},
        response_fetcher::fetcher,
        statics::NOT_FOUND_MANGA,
    },
};

use serenity::{
    builder::{CreateApplicationCommand, CreateEmbed},
    client::Context,
    model::{
        application::interaction::{
            application_command::ApplicationCommandInteraction, InteractionResponseType,
        },
        prelude::command::CommandOptionType,
    },
};

use tokio::task;
use tracing::info;

pub fn register(command: &mut CreateApplicationCommand) -> &mut CreateApplicationCommand {
    command
        .name("manga")
        .description("Fetches the details for a manga")
        .create_option(|option| {
            option
                .name("id")
                .description("Anilist ID")
                .kind(CommandOptionType::Integer)
                .min_int_value(1)
        })
        .create_option(|option| {
            option
                .name("name")
                .description("Search term")
                .kind(CommandOptionType::String)
        })
}

pub async fn run(ctx: &Context, interaction: &mut ApplicationCommandInteraction) {
    let user = &interaction.user;
    let arg = interaction.data.options[0].resolved.to_owned().unwrap();

    info!(
        "Got command 'manga' by user '{}' with args: {arg:#?}",
        user.name
    );

    let response: Option<Manga> = task::spawn_blocking(move || fetcher(Type::Manga, arg))
        .await
        .unwrap();

    let _manga_response = match response {
        None => {
            interaction
                .create_interaction_response(&ctx.http, |response| {
                    { response.kind(InteractionResponseType::ChannelMessageWithSource) }
                        .interaction_response_data(|m| m.content(NOT_FOUND_MANGA))
                })
                .await
        }
        Some(manga_response) => {
            // TODO: Refactor this to fetcher.rs

            let guild_members = get_current_guild_members(ctx, interaction);
            let also_manga = manga_response.clone();

            let scores = match guild_members.is_empty() {
                true => {
                    info!("No users found in guild");
                    None
                }
                false => {
                    let scores = task::spawn_blocking(move || {
                        get_guild_scores_for_media(also_manga, guild_members)
                    })
                    .await
                    .unwrap()
                    .await;
                    info!("Guild scores: {:#?}", scores);
                    Some(scores)
                }
            };

            interaction
                .create_interaction_response(&ctx.http, |response| {
                    { response.kind(InteractionResponseType::ChannelMessageWithSource) }
                        .interaction_response_data(|m| {
                            m.embed(|e| build_message_from_manga(manga_response, scores, e))
                        })
                })
                .await
        }
    };
}

fn build_message_from_manga(
    manga: Manga,
    scores: Option<HashMap<i64, u32>>,
    embed: &mut CreateEmbed,
) -> &mut CreateEmbed {
    embed
        .colour(manga.transform_color())
        .title(manga.transform_romaji_title())
        .description(manga.transform_description_and_mal_link())
        .fields(vec![
            ("Type", "Manga", true),                          // Field 0
            ("Status", &manga.transform_status(), true),      // Field 1
            ("Serialization", &manga.transform_date(), true), // Field 2
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
        .field("Staff", &manga.transform_staff(), false) // Field 10
        // .field("Mangadex Link", &manga.build_mangadex_link(), false) // Field 11
        .footer(|f| f.text(manga.transform_english_title()))
        .url(&manga.transform_anilist())
        .thumbnail(manga.transform_thumbnail());

    match scores {
        Some(scores) => {
            let mut score_string = String::new();
            for (user_id, score) in scores {
                score_string.push_str(&format!("<@{user_id}>: {score}\n"));
            }
            embed.field("Scores", &score_string, false)
        }
        None => embed,
    }
}
