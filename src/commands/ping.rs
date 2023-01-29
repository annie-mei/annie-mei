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

    // TODO: Convert response to PONG
    let _ping = interaction
        .create_interaction_response(&ctx.http, |response| {
            { response.kind(InteractionResponseType::ChannelMessageWithSource) }
                .interaction_response_data(|m| {
                    m.content(format!(
                        "Hello <@{}>! I'm Annie Mai, a bot that helps you find anime and manga!",
                        user.id
                    ))
                })
        })
        .await;
}
