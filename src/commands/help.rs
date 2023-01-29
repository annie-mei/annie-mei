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
                            .title(format!("Hello there {}!", user.mention()))
                            .description("Use these commands to interact with Annie Mai!")
                            .field("/anime", "Search for an anime", false)
                            .field("/manga", "Search for a manga", false)
                            .field("/songs", "Lookup an anime's songs", false)
                            .field("/register", "Tell me your Anilist username", false)
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
