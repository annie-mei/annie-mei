use serenity::{
    builder::CreateApplicationCommand,
    model::application::interaction::{
        application_command::ApplicationCommandInteraction, InteractionResponseType,
    },
    prelude::*,
};

pub fn register(command: &mut CreateApplicationCommand) -> &mut CreateApplicationCommand {
    command.name("ping").description("A ping command")
}

pub async fn run(ctx: &Context, interaction: &ApplicationCommandInteraction) {
    let user = &interaction.user;

    sentry::configure_scope(|scope| {
        let mut context = std::collections::BTreeMap::new();
        context.insert("Command".to_string(), "Register".into());
        scope.set_context("Ping", sentry::protocol::Context::Other(context));
        scope.set_user(Some(sentry::User {
            username: Some(user.name.to_string()),
            ..Default::default()
        }));
    });

    let _ping = interaction
        .create_interaction_response(&ctx.http, |response| {
            { response.kind(InteractionResponseType::ChannelMessageWithSource) }
                .interaction_response_data(|m| {
                    m.content(format!(
                        "Hello {}! I'm Annie Mei, a bot that helps you find anime and manga!",
                        user.mention()
                    ))
                })
        })
        .await;
}
