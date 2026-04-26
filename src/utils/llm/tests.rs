use super::*;

fn sample_config() -> GeminiClientConfig {
    GeminiClientConfig {
        api_key: "test-key".to_string(),
        base_url: "https://example.com/v1".to_string(),
        model: "test-model".to_string(),
        system_prompt: None,
        temperature: None,
    }
}

#[test]
fn build_messages_without_system_prompt() {
    let client = GeminiClient::new(sample_config()).unwrap();
    let messages = client.build_messages("hello");

    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].role, ChatRole::User);
    assert_eq!(messages[0].content, "hello");
}

#[test]
fn build_messages_with_system_prompt() {
    let mut config = sample_config();
    config.system_prompt = Some("You are a helpful bot.".to_string());
    let client = GeminiClient::new(config).unwrap();
    let messages = client.build_messages("hello");

    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0].role, ChatRole::System);
    assert_eq!(messages[0].content, "You are a helpful bot.");
    assert_eq!(messages[1].role, ChatRole::User);
    assert_eq!(messages[1].content, "hello");
}

#[test]
fn model_returns_configured_model_name() {
    let client = GeminiClient::new(sample_config()).unwrap();
    assert_eq!(client.model(), "test-model");
}

#[test]
fn with_temperature_sets_value() {
    let client = GeminiClient::new(sample_config())
        .unwrap()
        .with_temperature(0.7)
        .unwrap();
    assert_eq!(client.config.temperature, Some(0.7));
}

#[test]
fn with_temperature_rejects_nan() {
    let result = GeminiClient::new(sample_config())
        .unwrap()
        .with_temperature(f32::NAN);
    assert!(result.is_err());
}

#[test]
fn with_temperature_rejects_infinity() {
    let result = GeminiClient::new(sample_config())
        .unwrap()
        .with_temperature(f32::INFINITY);
    assert!(result.is_err());
}

#[test]
fn with_temperature_rejects_out_of_range() {
    let result = GeminiClient::new(sample_config())
        .unwrap()
        .with_temperature(2.5);
    assert!(result.is_err());

    let result = GeminiClient::new(sample_config())
        .unwrap()
        .with_temperature(-0.1);
    assert!(result.is_err());
}

#[test]
fn with_temperature_accepts_boundary_values() {
    let client = GeminiClient::new(sample_config())
        .unwrap()
        .with_temperature(0.0)
        .unwrap();
    assert_eq!(client.config.temperature, Some(0.0));

    let client = GeminiClient::new(sample_config())
        .unwrap()
        .with_temperature(2.0)
        .unwrap();
    assert_eq!(client.config.temperature, Some(2.0));
}

#[test]
fn chat_role_serializes_lowercase() {
    let json = serde_json::to_string(&ChatRole::System).unwrap();
    assert_eq!(json, "\"system\"");

    let json = serde_json::to_string(&ChatRole::User).unwrap();
    assert_eq!(json, "\"user\"");

    let json = serde_json::to_string(&ChatRole::Assistant).unwrap();
    assert_eq!(json, "\"assistant\"");
}

#[test]
fn chat_role_deserializes_lowercase() {
    let role: ChatRole = serde_json::from_str("\"system\"").unwrap();
    assert_eq!(role, ChatRole::System);

    let role: ChatRole = serde_json::from_str("\"user\"").unwrap();
    assert_eq!(role, ChatRole::User);

    let role: ChatRole = serde_json::from_str("\"assistant\"").unwrap();
    assert_eq!(role, ChatRole::Assistant);
}

#[test]
fn chat_message_roundtrip_serde() {
    let msg = ChatMessage {
        role: ChatRole::User,
        content: "hello world".to_string(),
    };
    let json = serde_json::to_string(&msg).unwrap();
    let deserialized: ChatMessage = serde_json::from_str(&json).unwrap();
    assert_eq!(msg, deserialized);
}

#[test]
fn chat_completion_response_deserializes() {
    let json = r#"{
        "choices": [{
            "index": 0,
            "message": {
                "role": "assistant",
                "content": "Hello! How can I help?"
            },
            "finish_reason": "stop"
        }],
        "model": "gemini-2.0-flash",
        "usage": {
            "prompt_tokens": 10,
            "completion_tokens": 8,
            "total_tokens": 18
        }
    }"#;

    let response: ChatCompletionResponse = serde_json::from_str(json).unwrap();
    assert_eq!(response.choices.len(), 1);
    assert_eq!(
        response.choices[0].message.content,
        "Hello! How can I help?"
    );
    assert_eq!(response.choices[0].message.role, "assistant");
    assert_eq!(response.choices[0].finish_reason.as_deref(), Some("stop"));
    assert_eq!(response.model.as_deref(), Some("gemini-2.0-flash"));
    assert_eq!(response.usage.as_ref().unwrap().total_tokens, Some(18));
}

#[test]
fn chat_completion_response_handles_minimal_json() {
    let json = r#"{
        "choices": [{
            "index": 0,
            "message": {
                "role": "assistant",
                "content": "Hi"
            },
            "finish_reason": null
        }]
    }"#;

    let response: ChatCompletionResponse = serde_json::from_str(json).unwrap();
    assert_eq!(response.choices.len(), 1);
    assert_eq!(response.choices[0].message.content, "Hi");
    assert!(response.model.is_none());
    assert!(response.usage.is_none());
}

#[test]
fn extract_reply_returns_first_choice_content() {
    let response = ChatCompletionResponse {
        choices: vec![Choice {
            index: 0,
            message: ChoiceMessage {
                role: "assistant".to_string(),
                content: "Hello!".to_string(),
            },
            finish_reason: Some("stop".to_string()),
        }],
        model: Some("test-model".to_string()),
        usage: None,
    };

    let reply = GeminiClient::extract_reply(response).unwrap();
    assert_eq!(reply, "Hello!");
}

#[test]
fn extract_reply_returns_error_on_empty_choices() {
    let response = ChatCompletionResponse {
        choices: vec![],
        model: None,
        usage: None,
    };

    let result = GeminiClient::extract_reply(response);
    assert!(result.is_err());
}

#[test]
fn request_body_omits_temperature_when_none() {
    let body = ChatCompletionRequest {
        model: "test".to_string(),
        messages: vec![],
        temperature: None,
    };
    let json = serde_json::to_string(&body).unwrap();
    assert!(!json.contains("temperature"));
}

#[test]
fn request_body_includes_temperature_when_set() {
    let body = ChatCompletionRequest {
        model: "test".to_string(),
        messages: vec![],
        temperature: Some(0.5),
    };
    let json = serde_json::to_string(&body).unwrap();
    assert!(json.contains("\"temperature\":0.5"));
}

#[test]
fn llm_error_display() {
    let err = LlmError::MissingEnvVar("GEMINI_API_KEY".to_string());
    assert_eq!(
        err.to_string(),
        "missing environment variable: GEMINI_API_KEY"
    );

    let err = LlmError::EmptyResponse;
    assert_eq!(err.to_string(), "response contained no choices");

    let err = LlmError::ApiError {
        status: 429,
        body: "rate limited".to_string(),
    };
    assert_eq!(err.to_string(), "API error (HTTP 429): rate limited");

    let err = LlmError::InvalidTemperature(3.0);
    assert!(err.to_string().contains("invalid temperature 3"));
}
