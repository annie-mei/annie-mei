//! OpenAI-compatible LLM client for AI capabilities.
//!
//! This module provides a pluggable, trait-based interface for interacting
//! with any OpenAI-compatible chat completions API.  The default
//! [`GeminiClient`] targets Google's Gemini API via its OpenAI compatibility
//! layer, but callers can implement [`LlmClient`] for any provider.
//!
//! ## Quick start
//!
//! ```ignore
//! use crate::utils::llm::{GeminiClient, LlmClient};
//!
//! let client = GeminiClient::from_env()?;
//! let response = client.chat("What anime should I watch?")?;
//! ```
//!
//! ## Environment variables
//!
//! | Variable         | Required | Description                              |
//! |------------------|----------|------------------------------------------|
//! | `GEMINI_API_KEY` | **yes**  | API key for the Gemini / OpenAI endpoint |
//! | `LLM_BASE_URL`   | no       | Base URL override for the API            |
//! | `LLM_MODEL`      | no       | Model name (default: `gemini-2.0-flash`) |

use std::env;
use std::fmt;
use std::time::Duration;

use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use tracing::{info, instrument};

use crate::utils::statics::{GEMINI_API_KEY, LLM_BASE_URL, LLM_MODEL};

const DEFAULT_BASE_URL: &str = "https://generativelanguage.googleapis.com/v1beta/openai";
const DEFAULT_MODEL: &str = "gemini-2.0-flash";
const DEFAULT_TIMEOUT_SECS: u64 = 30;

// ── Error type ───────────────────────────────────────────────────────

/// Errors that can occur during LLM operations.
#[derive(Debug)]
pub enum LlmError {
    /// A required environment variable is missing.
    MissingEnvVar(String),
    /// Failed to serialize the request body.
    Serialization(String),
    /// The HTTP request failed.
    Request(String),
    /// The API returned a non-success HTTP status.
    ApiError { status: u16, body: String },
    /// Failed to read the response body.
    ResponseBody(String),
    /// Failed to deserialize the response JSON.
    Deserialization { message: String, body: String },
    /// The response contained no choices.
    EmptyResponse,
    /// An invalid temperature value was provided.
    InvalidTemperature(f32),
}

impl fmt::Display for LlmError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LlmError::MissingEnvVar(var) => write!(f, "missing environment variable: {var}"),
            LlmError::Serialization(e) => write!(f, "request serialization failed: {e}"),
            LlmError::Request(e) => write!(f, "HTTP request failed: {e}"),
            LlmError::ApiError { status, body } => {
                write!(f, "API error (HTTP {status}): {body}")
            }
            LlmError::ResponseBody(e) => write!(f, "failed to read response body: {e}"),
            LlmError::Deserialization { message, .. } => {
                write!(f, "response deserialization failed: {message}")
            }
            LlmError::EmptyResponse => write!(f, "response contained no choices"),
            LlmError::InvalidTemperature(t) => {
                write!(
                    f,
                    "invalid temperature {t}: must be finite and in 0.0..=2.0"
                )
            }
        }
    }
}

impl std::error::Error for LlmError {}

// ── Request types ────────────────────────────────────────────────────

/// A single message in the chat conversation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChatMessage {
    pub role: ChatRole,
    pub content: String,
}

/// The role of a chat message participant.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ChatRole {
    System,
    User,
    Assistant,
}

/// Request body for the `/chat/completions` endpoint.
#[derive(Debug, Serialize)]
struct ChatCompletionRequest {
    model: String,
    messages: Vec<ChatMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
}

// ── Response types ───────────────────────────────────────────────────

/// Top-level response from the chat completions endpoint.
#[derive(Debug, Deserialize)]
pub struct ChatCompletionResponse {
    pub choices: Vec<Choice>,
    pub model: Option<String>,
    pub usage: Option<Usage>,
}

/// A single completion choice.
#[derive(Debug, Deserialize)]
pub struct Choice {
    pub index: u32,
    pub message: ChoiceMessage,
    pub finish_reason: Option<String>,
}

/// The message content within a choice.
#[derive(Debug, Deserialize)]
pub struct ChoiceMessage {
    pub role: String,
    pub content: String,
}

/// Token usage statistics.
#[derive(Debug, Deserialize)]
pub struct Usage {
    pub prompt_tokens: Option<u32>,
    pub completion_tokens: Option<u32>,
    pub total_tokens: Option<u32>,
}

// ── Trait ─────────────────────────────────────────────────────────────

/// Pluggable interface for LLM chat completions.
///
/// Implement this trait to swap providers, models, or add mock
/// implementations for testing.  The default production implementation
/// is [`GeminiClient`].
pub trait LlmClient: Send + Sync {
    /// Send a single user message and return the assistant's reply.
    ///
    /// If the implementation has a system prompt configured, it is
    /// automatically prepended to the conversation.
    fn chat(&self, user_message: &str) -> Result<String, LlmError>;

    /// Send a full conversation (multiple messages) and return the
    /// assistant's reply.
    ///
    /// The caller has full control over the message list — no system
    /// prompt is automatically prepended.
    fn chat_with_messages(&self, messages: &[ChatMessage]) -> Result<String, LlmError>;

    /// Return the model name this client is configured for.
    fn model(&self) -> &str;
}

// ── Gemini client ────────────────────────────────────────────────────

/// Configuration for a [`GeminiClient`].
///
/// Use [`GeminiClient::from_env`] for defaults, or build manually to
/// target a different provider / model.
#[derive(Clone)]
pub struct GeminiClientConfig {
    /// API key sent as a Bearer token.
    pub api_key: String,
    /// Base URL **without** the `/chat/completions` suffix.
    pub base_url: String,
    /// Model identifier (e.g. `"gemini-2.0-flash"`).
    pub model: String,
    /// Optional system prompt prepended to every `chat()` call.
    pub system_prompt: Option<String>,
    /// Optional temperature for sampling (0.0–2.0).
    pub temperature: Option<f32>,
}

impl fmt::Debug for GeminiClientConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GeminiClientConfig")
            .field("api_key", &"[REDACTED]")
            .field("base_url", &self.base_url)
            .field("model", &self.model)
            .field("system_prompt", &self.system_prompt)
            .field("temperature", &self.temperature)
            .finish()
    }
}

/// An OpenAI-compatible chat completions client targeting Gemini.
///
/// Implements [`LlmClient`] and can be reused across requests.
#[derive(Debug)]
pub struct GeminiClient {
    config: GeminiClientConfig,
    http: Client,
}

impl GeminiClient {
    /// Build a new client from an explicit config.
    pub fn new(config: GeminiClientConfig) -> Self {
        Self {
            config,
            http: Client::builder()
                .timeout(Duration::from_secs(DEFAULT_TIMEOUT_SECS))
                .build()
                .unwrap_or_default(),
        }
    }

    /// Build a client from environment variables.
    ///
    /// Returns an error if `GEMINI_API_KEY` is not set, allowing the
    /// bot to continue running without LLM capabilities.
    pub fn from_env() -> Result<Self, LlmError> {
        let api_key = env::var(GEMINI_API_KEY)
            .map_err(|_| LlmError::MissingEnvVar(GEMINI_API_KEY.to_string()))?;
        let base_url = env::var(LLM_BASE_URL).unwrap_or_else(|_| DEFAULT_BASE_URL.to_string());
        let model = env::var(LLM_MODEL).unwrap_or_else(|_| DEFAULT_MODEL.to_string());

        Ok(Self::new(GeminiClientConfig {
            api_key,
            base_url,
            model,
            system_prompt: None,
            temperature: None,
        }))
    }

    /// Build a client from environment variables with a system prompt.
    ///
    /// This is the primary constructor for command-specific AI workflows.
    /// Each workflow provides its own system prompt while sharing the
    /// global API key, base URL, and model configuration.
    pub fn from_env_with_system_prompt(system_prompt: impl Into<String>) -> Result<Self, LlmError> {
        let mut client = Self::from_env()?;
        client.config.system_prompt = Some(system_prompt.into());
        Ok(client)
    }

    /// Set the sampling temperature (0.0–2.0).
    ///
    /// Returns an error if the value is not finite or outside the
    /// supported range.
    pub fn with_temperature(mut self, temperature: f32) -> Result<Self, LlmError> {
        if !temperature.is_finite() || !(0.0..=2.0).contains(&temperature) {
            return Err(LlmError::InvalidTemperature(temperature));
        }
        self.config.temperature = Some(temperature);
        Ok(self)
    }

    // ── Internal helpers ─────────────────────────────────────────────

    /// Assemble the message list for a simple user-message call.
    #[instrument(name = "llm.build_messages", skip(self, user_message))]
    fn build_messages(&self, user_message: &str) -> Vec<ChatMessage> {
        let mut messages = Vec::with_capacity(2);

        if let Some(system_prompt) = &self.config.system_prompt {
            messages.push(ChatMessage {
                role: ChatRole::System,
                content: system_prompt.clone(),
            });
        }

        messages.push(ChatMessage {
            role: ChatRole::User,
            content: user_message.to_string(),
        });

        messages
    }

    /// Extract the assistant's reply text from a completion response.
    #[instrument(name = "llm.extract_reply", skip(response))]
    fn extract_reply(response: ChatCompletionResponse) -> Result<String, LlmError> {
        response
            .choices
            .into_iter()
            .next()
            .map(|choice| choice.message.content)
            .ok_or(LlmError::EmptyResponse)
    }

    /// POST to the chat completions endpoint and deserialise the response.
    #[instrument(
        name = "http.llm.send_request",
        skip(self, messages),
        fields(
            endpoint = %format!("{}/chat/completions", self.config.base_url),
            model = %self.config.model,
        )
    )]
    fn send_chat_completion(
        &self,
        messages: &[ChatMessage],
    ) -> Result<ChatCompletionResponse, LlmError> {
        let url = format!("{}/chat/completions", self.config.base_url);

        let body = ChatCompletionRequest {
            model: self.config.model.clone(),
            messages: messages.to_vec(),
            temperature: self.config.temperature,
        };

        let body_json =
            serde_json::to_string(&body).map_err(|e| LlmError::Serialization(e.to_string()))?;

        let response = self
            .http
            .post(&url)
            .header("Content-Type", "application/json")
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .body(body_json)
            .send()
            .map_err(|e| LlmError::Request(e.to_string()))?;

        let status = response.status();
        let text = response
            .text()
            .map_err(|e| LlmError::ResponseBody(e.to_string()))?;

        if !status.is_success() {
            return Err(LlmError::ApiError {
                status: status.as_u16(),
                body: text,
            });
        }

        serde_json::from_str::<ChatCompletionResponse>(&text).map_err(|e| {
            LlmError::Deserialization {
                message: e.to_string(),
                body: text,
            }
        })
    }
}

impl LlmClient for GeminiClient {
    #[instrument(
        name = "llm.chat",
        skip(self, user_message),
        fields(
            model = %self.config.model,
            has_system_prompt = self.config.system_prompt.is_some(),
        )
    )]
    fn chat(&self, user_message: &str) -> Result<String, LlmError> {
        let messages = self.build_messages(user_message);
        let response = self.send_chat_completion(&messages)?;

        info!(usage = ?response.usage, "Chat completion received");

        Self::extract_reply(response)
    }

    #[instrument(
        name = "llm.chat_with_messages",
        skip(self, messages),
        fields(
            model = %self.config.model,
            message_count = messages.len(),
        )
    )]
    fn chat_with_messages(&self, messages: &[ChatMessage]) -> Result<String, LlmError> {
        let response = self.send_chat_completion(messages)?;

        info!(usage = ?response.usage, "Chat completion received");

        Self::extract_reply(response)
    }

    fn model(&self) -> &str {
        &self.config.model
    }
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
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
        let client = GeminiClient::new(sample_config());
        let messages = client.build_messages("hello");

        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].role, ChatRole::User);
        assert_eq!(messages[0].content, "hello");
    }

    #[test]
    fn build_messages_with_system_prompt() {
        let mut config = sample_config();
        config.system_prompt = Some("You are a helpful bot.".to_string());
        let client = GeminiClient::new(config);
        let messages = client.build_messages("hello");

        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].role, ChatRole::System);
        assert_eq!(messages[0].content, "You are a helpful bot.");
        assert_eq!(messages[1].role, ChatRole::User);
        assert_eq!(messages[1].content, "hello");
    }

    #[test]
    fn model_returns_configured_model_name() {
        let client = GeminiClient::new(sample_config());
        assert_eq!(client.model(), "test-model");
    }

    #[test]
    fn with_temperature_sets_value() {
        let client = GeminiClient::new(sample_config())
            .with_temperature(0.7)
            .unwrap();
        assert_eq!(client.config.temperature, Some(0.7));
    }

    #[test]
    fn with_temperature_rejects_nan() {
        let result = GeminiClient::new(sample_config()).with_temperature(f32::NAN);
        assert!(result.is_err());
    }

    #[test]
    fn with_temperature_rejects_infinity() {
        let result = GeminiClient::new(sample_config()).with_temperature(f32::INFINITY);
        assert!(result.is_err());
    }

    #[test]
    fn with_temperature_rejects_out_of_range() {
        let result = GeminiClient::new(sample_config()).with_temperature(2.5);
        assert!(result.is_err());

        let result = GeminiClient::new(sample_config()).with_temperature(-0.1);
        assert!(result.is_err());
    }

    #[test]
    fn with_temperature_accepts_boundary_values() {
        let client = GeminiClient::new(sample_config())
            .with_temperature(0.0)
            .unwrap();
        assert_eq!(client.config.temperature, Some(0.0));

        let client = GeminiClient::new(sample_config())
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

    #[test]
    fn llm_error_display_all_variants() {
        let err = LlmError::Serialization("invalid JSON".to_string());
        assert!(err.to_string().contains("request serialization failed"));
        assert!(err.to_string().contains("invalid JSON"));

        let err = LlmError::Request("connection timeout".to_string());
        assert!(err.to_string().contains("HTTP request failed"));
        assert!(err.to_string().contains("connection timeout"));

        let err = LlmError::ResponseBody("stream error".to_string());
        assert!(err.to_string().contains("failed to read response body"));
        assert!(err.to_string().contains("stream error"));

        let err = LlmError::Deserialization {
            message: "unexpected field".to_string(),
            body: "{}".to_string(),
        };
        assert!(err.to_string().contains("response deserialization failed"));
        assert!(err.to_string().contains("unexpected field"));
    }

    #[test]
    fn llm_error_implements_error_trait() {
        let err = LlmError::EmptyResponse;
        let _error_trait: &dyn std::error::Error = &err;
    }

    #[test]
    fn gemini_client_config_debug_redacts_api_key() {
        let config = GeminiClientConfig {
            api_key: "super-secret-key-12345".to_string(),
            base_url: "https://example.com".to_string(),
            model: "test-model".to_string(),
            system_prompt: Some("You are a bot".to_string()),
            temperature: Some(0.8),
        };

        let debug_output = format!("{:?}", config);
        assert!(debug_output.contains("[REDACTED]"));
        assert!(!debug_output.contains("super-secret-key-12345"));
        assert!(debug_output.contains("https://example.com"));
        assert!(debug_output.contains("test-model"));
        assert!(debug_output.contains("You are a bot"));
        assert!(debug_output.contains("0.8"));
    }

    #[test]
    fn from_env_with_system_prompt_sets_prompt() {
        env::set_var(GEMINI_API_KEY, "test-api-key");
        env::set_var(LLM_BASE_URL, "https://test.example.com");
        env::set_var(LLM_MODEL, "test-model-v2");

        let client =
            GeminiClient::from_env_with_system_prompt("You are a helpful assistant.").unwrap();

        assert_eq!(
            client.config.system_prompt.as_deref(),
            Some("You are a helpful assistant.")
        );
        assert_eq!(client.config.base_url, "https://test.example.com");
        assert_eq!(client.config.model, "test-model-v2");

        env::remove_var(GEMINI_API_KEY);
        env::remove_var(LLM_BASE_URL);
        env::remove_var(LLM_MODEL);
    }

    #[test]
    fn from_env_uses_defaults_when_optional_vars_missing() {
        env::set_var(GEMINI_API_KEY, "test-key");
        env::remove_var(LLM_BASE_URL);
        env::remove_var(LLM_MODEL);

        let client = GeminiClient::from_env().unwrap();

        assert_eq!(client.config.base_url, DEFAULT_BASE_URL);
        assert_eq!(client.config.model, DEFAULT_MODEL);

        env::remove_var(GEMINI_API_KEY);
    }

    #[test]
    fn from_env_fails_without_api_key() {
        env::remove_var(GEMINI_API_KEY);

        let result = GeminiClient::from_env();
        assert!(result.is_err());
        match result {
            Err(LlmError::MissingEnvVar(var)) => assert_eq!(var, GEMINI_API_KEY),
            _ => panic!("Expected MissingEnvVar error"),
        }
    }

    #[test]
    fn with_temperature_accepts_mid_range_values() {
        let client = GeminiClient::new(sample_config())
            .with_temperature(1.0)
            .unwrap();
        assert_eq!(client.config.temperature, Some(1.0));

        let client = GeminiClient::new(sample_config())
            .with_temperature(0.5)
            .unwrap();
        assert_eq!(client.config.temperature, Some(0.5));

        let client = GeminiClient::new(sample_config())
            .with_temperature(1.5)
            .unwrap();
        assert_eq!(client.config.temperature, Some(1.5));
    }

    #[test]
    fn with_temperature_accepts_negative_zero() {
        let client = GeminiClient::new(sample_config())
            .with_temperature(-0.0)
            .unwrap();
        assert_eq!(client.config.temperature, Some(-0.0));
    }

    #[test]
    fn chat_message_with_empty_content() {
        let msg = ChatMessage {
            role: ChatRole::Assistant,
            content: "".to_string(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        let deserialized: ChatMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(msg, deserialized);
        assert_eq!(deserialized.content, "");
    }

    #[test]
    fn chat_message_with_multiline_content() {
        let msg = ChatMessage {
            role: ChatRole::System,
            content: "Line 1\nLine 2\nLine 3".to_string(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        let deserialized: ChatMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(msg, deserialized);
        assert!(deserialized.content.contains('\n'));
    }

    #[test]
    fn chat_message_with_special_characters() {
        let msg = ChatMessage {
            role: ChatRole::User,
            content: r#"Special chars: "quotes", 'apostrophes', \backslashes\, 日本語"#.to_string(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        let deserialized: ChatMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(msg, deserialized);
    }

    #[test]
    fn chat_completion_response_with_multiple_choices() {
        let json = r#"{
            "choices": [
                {
                    "index": 0,
                    "message": {"role": "assistant", "content": "First"},
                    "finish_reason": "stop"
                },
                {
                    "index": 1,
                    "message": {"role": "assistant", "content": "Second"},
                    "finish_reason": "stop"
                }
            ]
        }"#;

        let response: ChatCompletionResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.choices.len(), 2);
        assert_eq!(response.choices[0].message.content, "First");
        assert_eq!(response.choices[1].message.content, "Second");
    }

    #[test]
    fn extract_reply_returns_only_first_choice() {
        let response = ChatCompletionResponse {
            choices: vec![
                Choice {
                    index: 0,
                    message: ChoiceMessage {
                        role: "assistant".to_string(),
                        content: "First response".to_string(),
                    },
                    finish_reason: Some("stop".to_string()),
                },
                Choice {
                    index: 1,
                    message: ChoiceMessage {
                        role: "assistant".to_string(),
                        content: "Second response".to_string(),
                    },
                    finish_reason: Some("stop".to_string()),
                },
            ],
            model: None,
            usage: None,
        };

        let reply = GeminiClient::extract_reply(response).unwrap();
        assert_eq!(reply, "First response");
    }

    #[test]
    fn chat_completion_response_with_zero_tokens() {
        let json = r#"{
            "choices": [{
                "index": 0,
                "message": {"role": "assistant", "content": "Hi"},
                "finish_reason": "stop"
            }],
            "usage": {
                "prompt_tokens": 0,
                "completion_tokens": 0,
                "total_tokens": 0
            }
        }"#;

        let response: ChatCompletionResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.usage.as_ref().unwrap().total_tokens, Some(0));
    }

    #[test]
    fn build_messages_preserves_user_message_exactly() {
        let client = GeminiClient::new(sample_config());
        let input = "  whitespace  \n\ttabs\t  ";
        let messages = client.build_messages(input);

        assert_eq!(messages[0].content, input);
    }

    #[test]
    fn request_body_serializes_all_messages() {
        let messages = vec![
            ChatMessage {
                role: ChatRole::System,
                content: "system".to_string(),
            },
            ChatMessage {
                role: ChatRole::User,
                content: "user".to_string(),
            },
        ];

        let body = ChatCompletionRequest {
            model: "test".to_string(),
            messages: messages.clone(),
            temperature: Some(1.2),
        };

        let json = serde_json::to_string(&body).unwrap();
        assert!(json.contains("system"));
        assert!(json.contains("user"));
        assert!(json.contains("test"));
        assert!(json.contains("1.2"));
    }

    #[test]
    fn gemini_client_new_creates_valid_client() {
        let config = sample_config();
        let client = GeminiClient::new(config.clone());

        assert_eq!(client.config.api_key, config.api_key);
        assert_eq!(client.config.base_url, config.base_url);
        assert_eq!(client.config.model, config.model);
    }

    #[test]
    fn llm_client_trait_model_method() {
        let client = GeminiClient::new(sample_config());
        let trait_obj: &dyn LlmClient = &client;
        assert_eq!(trait_obj.model(), "test-model");
    }

    #[test]
    fn usage_deserialization_with_null_fields() {
        let json = r#"{
            "choices": [{
                "index": 0,
                "message": {"role": "assistant", "content": "Hi"},
                "finish_reason": "stop"
            }],
            "usage": {
                "prompt_tokens": null,
                "completion_tokens": null,
                "total_tokens": null
            }
        }"#;

        let response: ChatCompletionResponse = serde_json::from_str(json).unwrap();
        let usage = response.usage.as_ref().unwrap();
        assert_eq!(usage.prompt_tokens, None);
        assert_eq!(usage.completion_tokens, None);
        assert_eq!(usage.total_tokens, None);
    }

    #[test]
    fn choice_with_different_finish_reasons() {
        let json = r#"{
            "choices": [{
                "index": 0,
                "message": {"role": "assistant", "content": "Partial"},
                "finish_reason": "length"
            }]
        }"#;

        let response: ChatCompletionResponse = serde_json::from_str(json).unwrap();
        assert_eq!(
            response.choices[0].finish_reason.as_deref(),
            Some("length")
        );
    }
}