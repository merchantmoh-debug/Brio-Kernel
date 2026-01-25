use crate::inference::provider::LLMProvider;
use crate::inference::types::{ChatRequest, ChatResponse, InferenceError, Message, Role, Usage};
use async_trait::async_trait;
use reqwest::{Client, StatusCode, Url};
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::{debug, warn};

/// Default maximum number of retries for transient errors
const DEFAULT_MAX_RETRIES: u32 = 3;
/// Default base delay for exponential backoff (in milliseconds)
const DEFAULT_BASE_DELAY_MS: u64 = 1000;
/// Maximum delay cap (in milliseconds)
const MAX_DELAY_MS: u64 = 30000;
/// Default Anthropic API version
const DEFAULT_API_VERSION: &str = "2023-06-01";

// =============================================================================
// Anthropic API Types
// =============================================================================

#[derive(Serialize)]
struct AnthropicMessage {
    role: String,
    content: String,
}

#[derive(Serialize)]
struct AnthropicChatRequest {
    model: String,
    max_tokens: u32,
    messages: Vec<AnthropicMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<String>,
}

#[derive(Deserialize)]
struct AnthropicContent {
    text: String,
}

#[derive(Deserialize)]
struct AnthropicUsage {
    input_tokens: u32,
    output_tokens: u32,
}

#[derive(Deserialize)]
struct AnthropicChatResponse {
    content: Vec<AnthropicContent>,
    usage: Option<AnthropicUsage>,
}

// =============================================================================
// Configuration
// =============================================================================

/// Configuration for the Anthropic provider
pub struct AnthropicConfig {
    pub api_key: SecretString,
    pub base_url: Url,
    /// Maximum number of retries for rate limits and transient errors
    pub max_retries: Option<u32>,
    /// Base delay in milliseconds for exponential backoff
    pub base_delay_ms: Option<u64>,
    /// API version header value
    pub api_version: Option<String>,
    /// Maximum tokens to generate (required by Anthropic API)
    pub max_tokens: Option<u32>,
}

impl AnthropicConfig {
    /// Creates a new config with default settings
    pub fn new(api_key: SecretString, base_url: Url) -> Self {
        Self {
            api_key,
            base_url,
            max_retries: None,
            base_delay_ms: None,
            api_version: None,
            max_tokens: None,
        }
    }

    /// Creates a config with default Anthropic base URL
    pub fn with_api_key(api_key: SecretString) -> Self {
        Self::new(
            api_key,
            Url::parse("https://api.anthropic.com/v1/").expect("Valid Anthropic URL"),
        )
    }

    /// Sets the maximum number of retries
    pub fn with_max_retries(mut self, max_retries: u32) -> Self {
        self.max_retries = Some(max_retries);
        self
    }

    /// Sets the base delay for exponential backoff
    pub fn with_base_delay_ms(mut self, delay_ms: u64) -> Self {
        self.base_delay_ms = Some(delay_ms);
        self
    }

    /// Sets the API version
    pub fn with_api_version(mut self, version: String) -> Self {
        self.api_version = Some(version);
        self
    }

    /// Sets the maximum tokens to generate
    pub fn with_max_tokens(mut self, max_tokens: u32) -> Self {
        self.max_tokens = Some(max_tokens);
        self
    }
}

// =============================================================================
// Provider Implementation
// =============================================================================

pub struct AnthropicProvider {
    client: Client,
    config: AnthropicConfig,
    max_retries: u32,
    base_delay_ms: u64,
    api_version: String,
    max_tokens: u32,
}

impl AnthropicProvider {
    pub fn new(config: AnthropicConfig) -> Self {
        let max_retries = config.max_retries.unwrap_or(DEFAULT_MAX_RETRIES);
        let base_delay_ms = config.base_delay_ms.unwrap_or(DEFAULT_BASE_DELAY_MS);
        let api_version = config
            .api_version
            .clone()
            .unwrap_or_else(|| DEFAULT_API_VERSION.to_string());
        let max_tokens = config.max_tokens.unwrap_or(4096);

        Self {
            client: Client::new(),
            max_retries,
            base_delay_ms,
            api_version,
            max_tokens,
            config,
        }
    }

    /// Calculates the delay for a given retry attempt with jitter
    fn calculate_backoff_delay(&self, attempt: u32) -> Duration {
        let delay_ms = self.base_delay_ms.saturating_mul(1u64 << attempt);
        let capped_delay = delay_ms.min(MAX_DELAY_MS);
        let jitter = (capped_delay as f64 * 0.25 * rand_jitter()) as u64;
        Duration::from_millis(capped_delay + jitter)
    }

    /// Converts our Message type to Anthropic format, extracting system message
    fn prepare_messages(messages: &[Message]) -> (Option<String>, Vec<AnthropicMessage>) {
        let mut system_message = None;
        let mut anthropic_messages = Vec::new();

        for msg in messages {
            match msg.role {
                Role::System => {
                    // Anthropic uses a separate system field, not in messages array
                    system_message = Some(msg.content.clone());
                }
                Role::User => {
                    anthropic_messages.push(AnthropicMessage {
                        role: "user".to_string(),
                        content: msg.content.clone(),
                    });
                }
                Role::Assistant => {
                    anthropic_messages.push(AnthropicMessage {
                        role: "assistant".to_string(),
                        content: msg.content.clone(),
                    });
                }
            }
        }

        (system_message, anthropic_messages)
    }

    /// Makes a single request attempt
    async fn make_request(
        &self,
        provider_req: &AnthropicChatRequest,
    ) -> Result<ChatResponse, (InferenceError, bool)> {
        let url = self.config.base_url.join("messages").map_err(|e| {
            (
                InferenceError::ConfigError(format!("Invalid URL join: {}", e)),
                false,
            )
        })?;

        let res = self
            .client
            .post(url)
            .header("x-api-key", self.config.api_key.expose_secret())
            .header("anthropic-version", &self.api_version)
            .header("Content-Type", "application/json")
            .json(provider_req)
            .send()
            .await
            .map_err(|e| {
                (
                    InferenceError::NetworkError(e.to_string()),
                    true, // Retry network errors
                )
            })?;

        match res.status() {
            StatusCode::OK => {
                let body: AnthropicChatResponse = res.json().await.map_err(|e| {
                    (
                        InferenceError::ProviderError(format!("Parse error: {}", e)),
                        false,
                    )
                })?;

                let content = body
                    .content
                    .first()
                    .map(|c| c.text.clone())
                    .unwrap_or_default();

                Ok(ChatResponse {
                    content,
                    usage: body.usage.map(|u| Usage {
                        prompt_tokens: u.input_tokens,
                        completion_tokens: u.output_tokens,
                        total_tokens: u.input_tokens + u.output_tokens,
                    }),
                })
            }
            // Anthropic uses 529 for overloaded, 429 for rate limit
            StatusCode::TOO_MANY_REQUESTS => Err((InferenceError::RateLimit, true)),
            status if status.as_u16() == 529 => Err((InferenceError::RateLimit, true)),
            StatusCode::BAD_REQUEST => {
                let text = res.text().await.unwrap_or_default();
                if text.contains("context_length") || text.contains("max_tokens") {
                    Err((InferenceError::ContextLengthExceeded, false))
                } else {
                    Err((
                        InferenceError::ProviderError(format!("Bad Request: {}", text)),
                        false,
                    ))
                }
            }
            StatusCode::INTERNAL_SERVER_ERROR
            | StatusCode::BAD_GATEWAY
            | StatusCode::SERVICE_UNAVAILABLE
            | StatusCode::GATEWAY_TIMEOUT => {
                let status = res.status();
                let text = res.text().await.unwrap_or_default();
                Err((
                    InferenceError::ProviderError(format!("HTTP {}: {}", status, text)),
                    true,
                ))
            }
            _ => {
                let status = res.status();
                let text = res.text().await.unwrap_or_default();
                Err((
                    InferenceError::ProviderError(format!("HTTP {}: {}", status, text)),
                    false,
                ))
            }
        }
    }
}

#[async_trait]
impl LLMProvider for AnthropicProvider {
    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse, InferenceError> {
        let (system, messages) = Self::prepare_messages(&request.messages);

        let provider_req = AnthropicChatRequest {
            model: request.model,
            max_tokens: self.max_tokens,
            messages,
            system,
        };

        let mut last_error = InferenceError::NetworkError("No attempts made".to_string());

        for attempt in 0..=self.max_retries {
            match self.make_request(&provider_req).await {
                Ok(response) => return Ok(response),
                Err((error, should_retry)) => {
                    last_error = error;

                    if !should_retry || attempt >= self.max_retries {
                        break;
                    }

                    let delay = self.calculate_backoff_delay(attempt);
                    warn!(
                        attempt = attempt + 1,
                        max_retries = self.max_retries,
                        delay_ms = delay.as_millis() as u64,
                        error = %last_error,
                        "Anthropic request failed, retrying after backoff"
                    );
                    tokio::time::sleep(delay).await;
                }
            }
        }

        debug!(
            attempts = self.max_retries + 1,
            "All Anthropic retry attempts exhausted"
        );
        Err(last_error)
    }
}

/// Simple pseudo-random jitter between 0.0 and 1.0
fn rand_jitter() -> f64 {
    use std::time::SystemTime;
    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    (nanos % 1000) as f64 / 1000.0
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use reqwest::Url;
    use secrecy::SecretString;

    #[test]
    fn test_anthropic_config_creation() {
        let api_key = SecretString::new("test-key".into());
        let base_url = Url::parse("https://api.anthropic.com/v1/").unwrap();
        let config = AnthropicConfig::new(api_key, base_url.clone());
        assert_eq!(config.base_url, base_url);
        assert!(config.max_retries.is_none());
        assert!(config.base_delay_ms.is_none());
    }

    #[test]
    fn test_anthropic_config_with_api_key() {
        let api_key = SecretString::new("test-key".into());
        let config = AnthropicConfig::with_api_key(api_key);
        assert_eq!(config.base_url.as_str(), "https://api.anthropic.com/v1/");
    }

    #[test]
    fn test_anthropic_config_with_retries() {
        let api_key = SecretString::new("test-key".into());
        let base_url = Url::parse("https://api.anthropic.com/v1/").unwrap();
        let config = AnthropicConfig::new(api_key, base_url)
            .with_max_retries(5)
            .with_base_delay_ms(500)
            .with_max_tokens(8192);
        assert_eq!(config.max_retries, Some(5));
        assert_eq!(config.base_delay_ms, Some(500));
        assert_eq!(config.max_tokens, Some(8192));
    }

    #[test]
    fn test_anthropic_provider_new() {
        let api_key = SecretString::new("test-key".into());
        let base_url = Url::parse("https://api.anthropic.com/v1/").unwrap();
        let config = AnthropicConfig::new(api_key, base_url);
        let provider = AnthropicProvider::new(config);
        assert_eq!(provider.max_retries, DEFAULT_MAX_RETRIES);
        assert_eq!(provider.base_delay_ms, DEFAULT_BASE_DELAY_MS);
        assert_eq!(provider.api_version, DEFAULT_API_VERSION);
    }

    #[test]
    fn test_prepare_messages_extracts_system() {
        let messages = vec![
            Message {
                role: Role::System,
                content: "You are helpful.".to_string(),
            },
            Message {
                role: Role::User,
                content: "Hello!".to_string(),
            },
        ];

        let (system, msgs) = AnthropicProvider::prepare_messages(&messages);
        assert_eq!(system, Some("You are helpful.".to_string()));
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0].role, "user");
        assert_eq!(msgs[0].content, "Hello!");
    }

    #[test]
    fn test_prepare_messages_no_system() {
        let messages = vec![
            Message {
                role: Role::User,
                content: "Hello!".to_string(),
            },
            Message {
                role: Role::Assistant,
                content: "Hi there!".to_string(),
            },
        ];

        let (system, msgs) = AnthropicProvider::prepare_messages(&messages);
        assert!(system.is_none());
        assert_eq!(msgs.len(), 2);
    }

    #[test]
    fn test_backoff_delay_calculation() {
        let api_key = SecretString::new("test-key".into());
        let base_url = Url::parse("https://api.anthropic.com/v1/").unwrap();
        let config = AnthropicConfig::new(api_key, base_url).with_base_delay_ms(1000);
        let provider = AnthropicProvider::new(config);

        let delay0 = provider.calculate_backoff_delay(0);
        assert!(delay0.as_millis() >= 1000);
        assert!(delay0.as_millis() <= 1250);

        let delay1 = provider.calculate_backoff_delay(1);
        assert!(delay1.as_millis() >= 2000);
        assert!(delay1.as_millis() <= 2500);
    }
}
