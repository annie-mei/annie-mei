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
use std::future::Future;
use std::time::Duration;

use reqwest::Client;
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
    /// Failed to build the HTTP client.
    ClientBuild(String),
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
            LlmError::ClientBuild(e) => write!(f, "failed to build HTTP client: {e}"),
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
    fn chat(&self, user_message: &str) -> impl Future<Output = Result<String, LlmError>> + Send;

    /// Send a full conversation (multiple messages) and return the
    /// assistant's reply.
    ///
    /// The caller has full control over the message list — no system
    /// prompt is automatically prepended.
    fn chat_with_messages(
        &self,
        messages: &[ChatMessage],
    ) -> impl Future<Output = Result<String, LlmError>> + Send;

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
    ///
    /// Returns an error if the HTTP client cannot be constructed
    /// (e.g. TLS backend unavailable).
    pub fn new(config: GeminiClientConfig) -> Result<Self, LlmError> {
        let http = Client::builder()
            .timeout(Duration::from_secs(DEFAULT_TIMEOUT_SECS))
            .build()
            .map_err(|e| LlmError::ClientBuild(e.to_string()))?;

        Ok(Self { config, http })
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

        Self::new(GeminiClientConfig {
            api_key,
            base_url,
            model,
            system_prompt: None,
            temperature: None,
        })
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
    async fn send_chat_completion(
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
            .await
            .map_err(|e| LlmError::Request(e.to_string()))?;

        let status = response.status();
        let text = response
            .text()
            .await
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
    async fn chat(&self, user_message: &str) -> Result<String, LlmError> {
        let messages = self.build_messages(user_message);
        let response = self.send_chat_completion(&messages).await?;

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
    async fn chat_with_messages(&self, messages: &[ChatMessage]) -> Result<String, LlmError> {
        let response = self.send_chat_completion(messages).await?;

        info!(usage = ?response.usage, "Chat completion received");

        Self::extract_reply(response)
    }

    fn model(&self) -> &str {
        &self.config.model
    }
}

#[cfg(test)]
mod tests;
