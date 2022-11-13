use serenity::{
    builder::CreateApplicationCommand,
    model::application::interaction::{
        application_command::ApplicationCommandInteraction, InteractionResponseType,
    },
    prelude::*,
};

pub fn register(command: &mut CreateApplicationCommand) -> &mut CreateApplicationCommand {
    command.name("help").description("Shows how to use the bot")
}

pub async fn run(ctx: &Context, interaction: &ApplicationCommandInteraction) {
    let user = &interaction.user;

    let _help = interaction
        .create_interaction_response(&ctx.http, |response| {
            { response.kind(InteractionResponseType::ChannelMessageWithSource) }
                .interaction_response_data(|m| {
                    m.embed(|e| {
                        e.colour(0x00ff00)
                            .title(format!("Hello there {}!", user.name))
                            .description("Use these commands to interact with Annie Mai!")
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
                            .field("/songs", "Lookup an anime's songs", false)
                            .field("/help", "Show this message", false)
                            .field("/ping", "Check if I'm reachable", false)
                            .footer(|f| f.text("Annie Mai"))
                            .timestamp(chrono::Utc::now())
                            .thumbnail("attachment://mai.jpg")
                    })
                    .add_file("./mai.jpg")
                })
        })
        .await;
}
