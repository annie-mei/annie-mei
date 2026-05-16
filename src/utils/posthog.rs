//! Best-effort PostHog telemetry helpers.
//!
//! This module intentionally uses PostHog's capture API directly instead of a
//! Rust SDK so LLM Analytics payloads stay explicit and easy to test.

use std::{env, fmt, time::Duration};

use reqwest::Client;
use serde_json::{Value, json};
use tracing::{debug, info, instrument, warn};

use crate::utils::{
    statics::{ENV, POSTHOG_CAPTURE_LLM_CONTENT, POSTHOG_HOST, POSTHOG_PROJECT_API_KEY},
    tls::install_rustls_crypto_provider,
};

const DEFAULT_POSTHOG_HOST: &str = "https://us.i.posthog.com";
const DEFAULT_TIMEOUT_SECS: u64 = 5;

/// Per-call context that is safe to send to PostHog.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LlmTelemetryContext {
    /// Stable, salted hash for the Discord user. Never a raw Discord ID.
    pub distinct_id: Option<String>,
    /// Stable, salted hash for the Discord guild. Never a raw Discord ID.
    pub guild_id: Option<String>,
    /// Command or workflow that triggered the LLM call.
    pub command: Option<String>,
    /// Runtime environment, such as `development`, `staging`, or `production`.
    pub environment: Option<String>,
    /// Optional display-friendly input for LLM Analytics when content capture is enabled.
    pub input: Option<Value>,
}

/// Configuration for PostHog LLM Analytics capture.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PostHogConfig {
    pub project_api_key: String,
    pub host: String,
    pub capture_content: bool,
}

impl PostHogConfig {
    /// Load PostHog configuration from environment variables.
    ///
    /// Returns `None` when `POSTHOG_PROJECT_API_KEY` is absent so telemetry can
    /// be disabled without affecting bot startup or command handling.
    #[instrument(name = "posthog.config.from_env")]
    pub fn from_env() -> Option<Self> {
        let project_api_key = match env::var(POSTHOG_PROJECT_API_KEY) {
            Ok(value) if !value.trim().is_empty() => value,
            _ => return None,
        };

        let host = env::var(POSTHOG_HOST).unwrap_or_else(|_| DEFAULT_POSTHOG_HOST.to_string());
        let capture_content = env_flag(POSTHOG_CAPTURE_LLM_CONTENT, true);

        Some(Self {
            project_api_key,
            host,
            capture_content,
        })
    }

    fn capture_endpoint(&self) -> String {
        format!("{}/i/v0/e/", self.host.trim_end_matches('/'))
    }
}

/// Best-effort PostHog client for capture API events.
#[derive(Clone)]
pub struct PostHogClient {
    config: PostHogConfig,
    http: Client,
}

impl fmt::Debug for PostHogClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PostHogClient")
            .field("project_api_key", &"[REDACTED]")
            .field("host", &self.config.host)
            .field("capture_content", &self.config.capture_content)
            .finish()
    }
}

impl PostHogClient {
    #[instrument(name = "posthog.client.new", skip(config), fields(host = %config.host))]
    pub fn new(config: PostHogConfig) -> Result<Self, String> {
        install_rustls_crypto_provider();

        let http = Client::builder()
            .timeout(Duration::from_secs(DEFAULT_TIMEOUT_SECS))
            .build()
            .map_err(|error| error.to_string())?;

        Ok(Self { config, http })
    }

    #[instrument(name = "posthog.client.from_env")]
    pub fn from_env() -> Option<Self> {
        let Some(config) = PostHogConfig::from_env() else {
            debug!("PostHog project API key not configured; LLM analytics disabled");
            return None;
        };

        match Self::new(config) {
            Ok(client) => {
                info!(host = %client.config.host, "PostHog LLM analytics enabled");
                Some(client)
            }
            Err(error) => {
                warn!(error = %error, "PostHog client unavailable; LLM analytics disabled");
                None
            }
        }
    }

    /// Send a capture event to PostHog.
    ///
    /// Errors are logged and swallowed by callers; telemetry must never break a
    /// Discord command.
    #[instrument(name = "posthog.capture", skip(self, event))]
    pub async fn capture(&self, event: Value) -> Result<(), String> {
        let summary = CaptureLogSummary::from_event(&event);
        let body = serde_json::to_string(&event).map_err(|error| error.to_string())?;
        let endpoint = self.config.capture_endpoint();

        info!(
            endpoint = %endpoint,
            event = summary.event.as_deref(),
            ai_trace_id = summary.ai_trace_id.as_deref(),
            ai_model = summary.ai_model.as_deref(),
            environment = summary.environment.as_deref(),
            command = summary.command.as_deref(),
            "Sending PostHog capture event"
        );

        let response = match self
            .http
            .post(&endpoint)
            .header("Content-Type", "application/json")
            .body(body)
            .send()
            .await
        {
            Ok(response) => response,
            Err(error) => {
                warn!(
                    error = %error,
                    event = summary.event.as_deref(),
                    ai_trace_id = summary.ai_trace_id.as_deref(),
                    "PostHog capture request failed"
                );
                return Err(error.to_string());
            }
        };

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            warn!(
                http_status = status.as_u16(),
                event = summary.event.as_deref(),
                ai_trace_id = summary.ai_trace_id.as_deref(),
                response_body_length = body.len(),
                "PostHog capture event rejected"
            );
            return Err(format!("PostHog capture failed with HTTP {status}: {body}"));
        }

        info!(
            http_status = status.as_u16(),
            event = summary.event.as_deref(),
            ai_trace_id = summary.ai_trace_id.as_deref(),
            "PostHog capture event sent"
        );
        Ok(())
    }

    /// Build a `$ai_generation` event payload for PostHog LLM Analytics.
    #[allow(clippy::too_many_arguments)]
    #[instrument(
        name = "posthog.build_ai_generation",
        skip(self, input, output_choices, error)
    )]
    pub fn build_ai_generation_event(
        &self,
        context: &LlmTelemetryContext,
        trace_id: &str,
        model: &str,
        provider: &str,
        latency_seconds: f64,
        input: Option<Value>,
        output_choices: Option<Value>,
        input_tokens: Option<u32>,
        output_tokens: Option<u32>,
        total_tokens: Option<u32>,
        error: Option<&str>,
    ) -> Value {
        build_ai_generation_event(
            &self.config.project_api_key,
            self.config.capture_content,
            context,
            trace_id,
            model,
            provider,
            latency_seconds,
            input,
            output_choices,
            input_tokens,
            output_tokens,
            total_tokens,
            error,
        )
    }
}

#[derive(Debug, Default)]
struct CaptureLogSummary {
    event: Option<String>,
    ai_trace_id: Option<String>,
    ai_model: Option<String>,
    environment: Option<String>,
    command: Option<String>,
}

impl CaptureLogSummary {
    #[instrument(name = "posthog.capture_log_summary.from_event", skip(event))]
    fn from_event(event: &Value) -> Self {
        let properties = event.get("properties");

        Self {
            event: event
                .get("event")
                .and_then(Value::as_str)
                .map(ToString::to_string),
            ai_trace_id: properties
                .and_then(|properties| properties.get("$ai_trace_id"))
                .and_then(Value::as_str)
                .map(ToString::to_string),
            ai_model: properties
                .and_then(|properties| properties.get("$ai_model"))
                .and_then(Value::as_str)
                .map(ToString::to_string),
            environment: properties
                .and_then(|properties| properties.get("environment"))
                .and_then(Value::as_str)
                .map(ToString::to_string),
            command: properties
                .and_then(|properties| properties.get("command"))
                .and_then(Value::as_str)
                .map(ToString::to_string),
        }
    }
}

/// Testable payload construction for PostHog LLM Analytics.
#[allow(clippy::too_many_arguments)]
pub fn build_ai_generation_event(
    project_api_key: &str,
    capture_content: bool,
    context: &LlmTelemetryContext,
    trace_id: &str,
    model: &str,
    provider: &str,
    latency_seconds: f64,
    input: Option<Value>,
    output_choices: Option<Value>,
    input_tokens: Option<u32>,
    output_tokens: Option<u32>,
    total_tokens: Option<u32>,
    error: Option<&str>,
) -> Value {
    let distinct_id = context
        .distinct_id
        .as_deref()
        .unwrap_or("annie-mei-unknown-user");

    let mut properties = serde_json::Map::new();
    properties.insert("distinct_id".to_string(), json!(distinct_id));
    properties.insert("$ai_trace_id".to_string(), json!(trace_id));
    properties.insert("$ai_model".to_string(), json!(model));
    properties.insert("$ai_provider".to_string(), json!(provider));
    properties.insert("$ai_latency".to_string(), json!(latency_seconds));
    properties.insert("$ai_stream".to_string(), json!(false));

    if let Some(command) = &context.command {
        properties.insert("command".to_string(), json!(command));
    }

    if let Some(environment) = context
        .environment
        .as_deref()
        .or(option_env(ENV).as_deref())
    {
        properties.insert("environment".to_string(), json!(environment));
    }

    if let Some(guild_id) = &context.guild_id {
        properties.insert("guild_id".to_string(), json!(guild_id));
    }

    if let Some(tokens) = input_tokens {
        properties.insert("$ai_input_tokens".to_string(), json!(tokens));
    }

    if let Some(tokens) = output_tokens {
        properties.insert("$ai_output_tokens".to_string(), json!(tokens));
    }

    if let Some(tokens) = total_tokens {
        properties.insert("$ai_total_tokens".to_string(), json!(tokens));
    }

    if let Some(error) = error {
        properties.insert("$ai_error".to_string(), json!(error));
        properties.insert("success".to_string(), json!(false));
    } else {
        properties.insert("success".to_string(), json!(true));
    }

    if capture_content {
        if let Some(input) = input {
            properties.insert("$ai_input".to_string(), input);
        }
        if let Some(output_choices) = output_choices {
            properties.insert("$ai_output_choices".to_string(), output_choices);
        }
    }

    json!({
        "api_key": project_api_key,
        "event": "$ai_generation",
        "properties": properties,
    })
}

fn option_env(name: &str) -> Option<String> {
    env::var(name).ok().filter(|value| !value.trim().is_empty())
}

fn env_flag(name: &str, default: bool) -> bool {
    match env::var(name) {
        Ok(value) => matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "y" | "on"
        ),
        Err(_) => default,
    }
}

#[cfg(test)]
mod tests {
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
}
