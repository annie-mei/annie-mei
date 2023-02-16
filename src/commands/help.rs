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

    sentry::configure_scope(|scope| {
        let mut context = std::collections::BTreeMap::new();
        context.insert("Command".to_string(), "Register".into());
        scope.set_context("Help", sentry::protocol::Context::Other(context));
        scope.set_user(Some(sentry::User {
            username: Some(user.name.to_string()),
            ..Default::default()
        }));
    });

    let _help = interaction
        .create_interaction_response(&ctx.http, |response| {
            { response.kind(InteractionResponseType::ChannelMessageWithSource) }
                .interaction_response_data(|m| {
                    m.embed(|e| {
                        e.colour(0x00ff00)
                            .title(format!("Hello there {}!", user.mention()))
                            .description("Use these commands to interact with Annie Mei!")
                            .field("/anime", "Search for an anime", false)
                            .field("/manga", "Search for a manga", false)
                            .field("/songs", "Lookup an anime's songs", false)
                            .field("/register", "Tell me your Anilist username", false)
                            .field("/help", "Show this message", false)
                            .field("/ping", "Check if I'm reachable", false)
                            .footer(|f| f.text("Annie Mei"))
                            .timestamp(chrono::Utc::now())
                            .thumbnail("attachment://mei.jpg")
                    })
                    .add_file("./mei.jpg")
                })
        })
        .await;
}
