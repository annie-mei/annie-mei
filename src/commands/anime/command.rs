use crate::{
    models::{anilist_anime::Anime, media_type::MediaType as Type, transformers::Transformers},
    utils::{response_fetcher::fetcher, statics::NOT_FOUND_ANIME},
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
        .name("anime")
        .description("Fetches the details for an anime")
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
        "Got command 'anime' by user '{}' with args: {:#?}",
        user.name, arg
    );

    let response = task::spawn_blocking(move || fetcher(Type::Anime, arg))
        .await
        .unwrap();

    let _anime_response = match response {
        None => {
            interaction
                .create_interaction_response(&ctx.http, |response| {
                    { response.kind(InteractionResponseType::ChannelMessageWithSource) }
                        .interaction_response_data(|m| m.content(NOT_FOUND_ANIME))
                })
                .await
        }
        Some(anime_response) => {
            interaction
                .create_interaction_response(&ctx.http, |response| {
                    { response.kind(InteractionResponseType::ChannelMessageWithSource) }
                        .interaction_response_data(|m| {
                            m.embed(|e| build_message_from_anime(anime_response, e))
                        })
                })
                .await
        }
    };
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
