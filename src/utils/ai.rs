//! OpenAI-compatible LLM client for AI capabilities.
//!
//! This module provides a pluggable interface for interacting with any
//! OpenAI-compatible chat completions API.  The default configuration
//! targets Google's Gemini API via its OpenAI compatibility layer, but
//! callers can override the base URL, model, and system prompt to use
//! any compatible provider.
//!
//! ## Quick start
//!
//! ```ignore
//! use crate::utils::ai::{ChatClient, ChatClientConfig};
//!
//! let client = ChatClient::from_env();
//! let response = client.chat("What anime should I watch?");
//! ```
//!
//! ## Environment variables
//!
//! | Variable            | Required | Description                              |
//! |---------------------|----------|------------------------------------------|
//! | `GEMINI_API_KEY`    | **yes**  | API key for the Gemini / OpenAI endpoint |
//! | `AI_MODEL`          | no       | Model name (default: `gemini-2.0-flash`) |
//! | `AI_BASE_URL`       | no       | Base URL for the API                     |

use std::env;

use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use tracing::{info, instrument};

use crate::utils::statics::{AI_BASE_URL, AI_MODEL, GEMINI_API_KEY};

const DEFAULT_BASE_URL: &str = "https://generativelanguage.googleapis.com/v1beta/openai";
const DEFAULT_MODEL: &str = "gemini-2.0-flash";

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

// ── Client ───────────────────────────────────────────────────────────

/// Configuration for a [`ChatClient`].
///
/// Use [`ChatClientConfig::default`] for Gemini defaults, or customise
/// individual fields to target a different provider / model.
#[derive(Debug, Clone)]
pub struct ChatClientConfig {
    /// API key sent as a Bearer token.
    pub api_key: String,
    /// Base URL **without** the `/chat/completions` suffix.
    pub base_url: String,
    /// Model identifier (e.g. `"gemini-2.0-flash"`).
    pub model: String,
    /// Optional system prompt prepended to every request.
    pub system_prompt: Option<String>,
}

/// An OpenAI-compatible chat completions client.
///
/// Designed to be cheap to construct and reusable across requests.
/// Holds a [`reqwest::blocking::Client`] internally.
#[derive(Debug)]
pub struct ChatClient {
    config: ChatClientConfig,
    http: Client,
}

impl ChatClient {
    /// Build a new client from an explicit config.
    pub fn new(config: ChatClientConfig) -> Self {
        Self {
            config,
            http: Client::new(),
        }
    }

    /// Build a client from environment variables.
    ///
    /// # Panics
    ///
    /// Panics if `GEMINI_API_KEY` is not set.
    pub fn from_env() -> Self {
        let api_key =
            env::var(GEMINI_API_KEY).expect("Expected a Gemini API key in the environment");
        let base_url = env::var(AI_BASE_URL).unwrap_or_else(|_| DEFAULT_BASE_URL.to_string());
        let model = env::var(AI_MODEL).unwrap_or_else(|_| DEFAULT_MODEL.to_string());

        Self::new(ChatClientConfig {
            api_key,
            base_url,
            model,
            system_prompt: None,
        })
    }

    /// Build a client from environment variables with a system prompt.
    ///
    /// This is the primary constructor for command-specific AI workflows.
    /// Each workflow provides its own system prompt while sharing the
    /// global API key, base URL, and model configuration.
    ///
    /// # Panics
    ///
    /// Panics if `GEMINI_API_KEY` is not set.
    pub fn from_env_with_system_prompt(system_prompt: impl Into<String>) -> Self {
        let mut client = Self::from_env();
        client.config.system_prompt = Some(system_prompt.into());
        client
    }

    /// Return the model name this client is configured for.
    pub fn model(&self) -> &str {
        &self.config.model
    }

    /// Send a single user message and return the assistant's reply.
    ///
    /// If a system prompt was configured, it is automatically prepended.
    ///
    /// Returns `None` if the API call fails or the response contains no
    /// choices.
    #[instrument(
        name = "ai.chat",
        skip(self, user_message),
        fields(
            model = %self.config.model,
            has_system_prompt = self.config.system_prompt.is_some(),
        )
    )]
    pub fn chat(&self, user_message: &str) -> Option<String> {
        let messages = self.build_messages(user_message);
        let response = self.send_chat_completion(&messages)?;

        let content = response
            .choices
            .into_iter()
            .next()
            .map(|choice| choice.message.content);

        info!(
            usage = ?response.usage,
            "Chat completion received"
        );

        content
    }

    /// Send a full conversation (multiple messages) and return the
    /// assistant's reply.
    ///
    /// The system prompt is **not** automatically prepended here — the
    /// caller has full control over the message list.
    #[instrument(
        name = "ai.chat_with_messages",
        skip(self, messages),
        fields(
            model = %self.config.model,
            message_count = messages.len(),
        )
    )]
    pub fn chat_with_messages(&self, messages: &[ChatMessage]) -> Option<String> {
        let response = self.send_chat_completion(messages)?;

        let content = response
            .choices
            .into_iter()
            .next()
            .map(|choice| choice.message.content);

        info!(
            usage = ?response.usage,
            "Chat completion received"
        );

        content
    }

    // ── Internal helpers ─────────────────────────────────────────────

    /// Assemble the message list for a simple user-message call.
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

    /// POST to the chat completions endpoint and deserialise the response.
    #[instrument(
        name = "http.ai.send_request",
        skip(self, messages),
        fields(
            endpoint = %format!("{}/chat/completions", self.config.base_url),
            model = %self.config.model,
        )
    )]
    fn send_chat_completion(&self, messages: &[ChatMessage]) -> Option<ChatCompletionResponse> {
        let url = format!("{}/chat/completions", self.config.base_url);

        let body = ChatCompletionRequest {
            model: self.config.model.clone(),
            messages: messages.to_vec(),
        };

        let body_json = match serde_json::to_string(&body) {
            Ok(json) => json,
            Err(e) => {
                tracing::error!(error = %e, "Failed to serialize chat completion request");
                return None;
            }
        };

        let result = self
            .http
            .post(&url)
            .header("Content-Type", "application/json")
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .body(body_json)
            .send();

        match result {
            Ok(response) => {
                let text = match response.text() {
                    Ok(t) => t,
                    Err(e) => {
                        tracing::error!(error = %e, "Failed to read chat completion response body");
                        return None;
                    }
                };
                match serde_json::from_str::<ChatCompletionResponse>(&text) {
                    Ok(completion) => Some(completion),
                    Err(e) => {
                        tracing::error!(error = %e, response_body = %text, "Failed to deserialize chat completion response");
                        None
                    }
                }
            }
            Err(e) => {
                tracing::error!(error = %e, "Failed to send chat completion request");
                None
            }
        }
    }
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_config() -> ChatClientConfig {
        ChatClientConfig {
            api_key: "test-key".to_string(),
            base_url: "https://example.com/v1".to_string(),
            model: "test-model".to_string(),
            system_prompt: None,
        }
    }

    #[test]
    fn build_messages_without_system_prompt() {
        let client = ChatClient::new(sample_config());
        let messages = client.build_messages("hello");

        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].role, ChatRole::User);
        assert_eq!(messages[0].content, "hello");
    }

    #[test]
    fn build_messages_with_system_prompt() {
        let mut config = sample_config();
        config.system_prompt = Some("You are a helpful bot.".to_string());
        let client = ChatClient::new(config);
        let messages = client.build_messages("hello");

        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].role, ChatRole::System);
        assert_eq!(messages[0].content, "You are a helpful bot.");
        assert_eq!(messages[1].role, ChatRole::User);
        assert_eq!(messages[1].content, "hello");
    }

    #[test]
    fn model_returns_configured_model_name() {
        let client = ChatClient::new(sample_config());
        assert_eq!(client.model(), "test-model");
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
}
