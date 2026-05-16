use serde_json::json;

use super::*;

#[test]
fn ai_generation_payload_excludes_content_by_default() {
    let context = LlmTelemetryContext {
        distinct_id: Some("user_hash".to_string()),
        guild_id: Some("guild_hash".to_string()),
        command: Some("search".to_string()),
        environment: Some("test".to_string()),
        input: None,
    };

    let event = build_ai_generation_event(
        "ph_key",
        false,
        &context,
        "trace-id",
        "gemini-2.0-flash",
        "gemini",
        1.25,
        Some(json!([{ "role": "user", "content": "raw prompt" }])),
        Some(json!([{ "role": "assistant", "content": "raw output" }])),
        Some(10),
        Some(20),
        Some(30),
        None,
    );

    let properties = event["properties"].as_object().unwrap();
    assert_eq!(event["api_key"], "ph_key");
    assert_eq!(event["event"], "$ai_generation");
    assert_eq!(event["distinct_id"], "user_hash");
    assert_eq!(properties["distinct_id"], "user_hash");
    assert_eq!(properties["guild_id"], "guild_hash");
    assert_eq!(properties["command"], "search");
    assert_eq!(properties["environment"], "test");
    assert_eq!(properties["$ai_model"], "gemini-2.0-flash");
    assert_eq!(properties["$ai_provider"], "gemini");
    assert_eq!(properties["$ai_input_tokens"], 10);
    assert_eq!(properties["$ai_output_tokens"], 20);
    assert_eq!(properties["$ai_total_tokens"], 30);
    assert!(!properties.contains_key("$ai_input"));
    assert!(!properties.contains_key("$ai_output_choices"));
}

#[test]
fn ai_generation_payload_includes_content_when_enabled() {
    let event = build_ai_generation_event(
        "ph_key",
        true,
        &LlmTelemetryContext::default(),
        "trace-id",
        "gemini-2.0-flash",
        "gemini",
        1.25,
        Some(json!([{ "role": "user", "content": "raw prompt" }])),
        Some(json!([{ "role": "assistant", "content": "raw output" }])),
        None,
        None,
        None,
        None,
    );

    let properties = event["properties"].as_object().unwrap();
    assert_eq!(properties["$ai_input"][0]["content"], "raw prompt");
    assert_eq!(properties["$ai_output_choices"][0]["content"], "raw output");
}

#[test]
fn ai_generation_payload_records_errors_without_content() {
    let event = build_ai_generation_event(
        "ph_key",
        false,
        &LlmTelemetryContext::default(),
        "trace-id",
        "gemini-2.0-flash",
        "gemini",
        0.5,
        Some(json!([{ "role": "user", "content": "raw prompt" }])),
        None,
        None,
        None,
        None,
        Some("HTTP request failed"),
    );

    let properties = event["properties"].as_object().unwrap();
    assert!(!properties["success"].as_bool().unwrap());
    assert_eq!(properties["$ai_error"], "HTTP request failed");
    assert!(!properties.contains_key("$ai_input"));
}

#[test]
fn command_hit_payload_includes_safe_command_context() {
    let context = CommandTelemetryContext {
        distinct_id: Some("user_hash".to_string()),
        guild_id: Some("guild_hash".to_string()),
        command: "anime".to_string(),
        environment: Some("test".to_string()),
        bot_version: "2.19.3".to_string(),
        source: "discord".to_string(),
        is_dm: false,
        channel_nsfw: true,
    };

    let event = build_command_hit_event("ph_key", &context);
    let properties = event["properties"].as_object().unwrap();

    assert_eq!(event["api_key"], "ph_key");
    assert_eq!(event["event"], "discord_command_hit");
    assert_eq!(event["distinct_id"], "user_hash");
    assert_eq!(properties["distinct_id"], "user_hash");
    assert_eq!(properties["guild_id"], "guild_hash");
    assert_eq!(properties["command"], "anime");
    assert_eq!(properties["environment"], "test");
    assert_eq!(properties["bot_version"], "2.19.3");
    assert_eq!(properties["source"], "discord");
    assert_eq!(properties["interaction_type"], "slash_command");
    assert_eq!(properties["is_dm"], false);
    assert_eq!(properties["channel_nsfw"], true);
}
