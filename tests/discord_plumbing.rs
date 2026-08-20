use annie_mei::commands::{BotCommand, ping};
use httpmock::{Method::POST, MockServer};
use serde_json::json;
use serenity::{http::HttpBuilder, model::application::Interaction};

#[tokio::test]
async fn ping_fixture_routes_and_sends_interaction_callback() {
    let interaction: Interaction =
        serde_json::from_str(include_str!("fixtures/interactions/ping.json"))
            .expect("fixture should deserialize as a Serenity interaction");
    let command = interaction
        .into_command()
        .expect("fixture should contain a command interaction");

    let routed_command = BotCommand::from_name(&command.data.name);
    assert_eq!(routed_command, Some(BotCommand::Ping));

    let server = MockServer::start_async().await;
    let callback = server
        .mock_async(|when, then| {
            when.method(POST)
                .path(
                    "/api/v10/interactions/100000000000000001/\
                     synthetic-interaction-token/callback",
                )
                .json_body(json!({
                    "type": 4,
                    "data": {
                        "content": "Hi <@100000000000000005>! I'm Annie Mei, awake and ready to help with anime, manga, recommendations, and theme songs.",
                        "attachments": []
                    }
                }));
            then.status(204);
        })
        .await;
    let http = HttpBuilder::new("synthetic-bot-token")
        .proxy(server.base_url())
        .ratelimiter_disabled(true)
        .build();

    ping::run_with_http(&http, &command)
        .await
        .expect("ping adapter should send the callback");

    callback.assert_async().await;
}
