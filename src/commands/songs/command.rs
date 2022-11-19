use crate::{
    commands::songs::fetcher::fetcher as SongFetcher, models::mal_response::MalResponse,
    utils::statics::NOT_FOUND_ANIME,
};

use serenity::{
    builder::{CreateApplicationCommand, CreateEmbed},
    client::Context,
    framework::standard::{Args, Delimiter},
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
        .name("songs")
        .description("Fetches the songs of an anime")
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

pub async fn run(ctx: &Context, interaction: &ApplicationCommandInteraction) {
    let user = &interaction.user;
    // Ignores the second value
    let arg = interaction.data.options[0]
        .value
        .clone()
        .unwrap()
        .to_string();

    info!(
        "Got command 'songs' by user '{}' with args: {:#?}",
        user.name,
        Args::new(arg.as_str(), &[Delimiter::Single(' ')])
    );

    // TODO: Remove this hack
    let args = Args::new(
        format!("songs {}", arg.as_str()).as_str(),
        &[Delimiter::Single(' ')],
    );
    let response = task::spawn_blocking(|| SongFetcher(args)).await.unwrap();

    let spotify_url = crate::utils::spotify::get_song_url(
        "Chainsaw Blood".to_owned(),
        "君の知らない物語".to_owned(),
        "Vaundy".to_owned(),
    )
    .await
    .unwrap_or_else(|| "None".to_owned());

    info!("Spotify results: {:#?}", spotify_url);

    let _songs_response = match response {
        None => {
            interaction
                .create_interaction_response(&ctx.http, |response| {
                    { response.kind(InteractionResponseType::ChannelMessageWithSource) }
                        .interaction_response_data(|m| m.content(NOT_FOUND_ANIME))
                })
                .await
        }
        Some(song_response) => {
            interaction
                .create_interaction_response(&ctx.http, |response| {
                    { response.kind(InteractionResponseType::ChannelMessageWithSource) }
                        .interaction_response_data(|m| {
                            m.embed(|e| build_message_from_song_response(song_response, e))
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
