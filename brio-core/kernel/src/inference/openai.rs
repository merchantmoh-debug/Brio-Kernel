use crate::inference::provider::LLMProvider;
use crate::inference::types::{ChatRequest, ChatResponse, InferenceError, Message, Usage};
use anyhow::Result;
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

#[derive(Serialize)]
struct OpenAIChatRequest {
    model: String,
    messages: Vec<Message>,
}

#[derive(Deserialize)]
struct OpenAIChoice {
    message: Message,
}

#[derive(Deserialize)]
struct OpenAIUsage {
    prompt_tokens: u32,
    completion_tokens: u32,
    total_tokens: u32,
}

#[derive(Deserialize)]
struct OpenAIChatResponse {
    choices: Vec<OpenAIChoice>,
    usage: Option<OpenAIUsage>,
}

/// Configuration for the OpenAI provider
pub struct OpenAIConfig {
    pub api_key: SecretString,
    pub base_url: Url,
    /// Maximum number of retries for rate limits and transient errors
    pub max_retries: Option<u32>,
    /// Base delay in milliseconds for exponential backoff
    pub base_delay_ms: Option<u64>,
}

impl OpenAIConfig {
    /// Creates a new config with default retry settings
    pub fn new(api_key: SecretString, base_url: Url) -> Self {
        Self {
            api_key,
            base_url,
            max_retries: None,
            base_delay_ms: None,
        }
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
}

pub struct OpenAIProvider {
    client: Client,
    config: OpenAIConfig,
    max_retries: u32,
    base_delay_ms: u64,
}

impl OpenAIProvider {
    pub fn new(config: OpenAIConfig) -> Self {
        let max_retries = config.max_retries.unwrap_or(DEFAULT_MAX_RETRIES);
        let base_delay_ms = config.base_delay_ms.unwrap_or(DEFAULT_BASE_DELAY_MS);
        Self {
            client: Client::new(),
            max_retries,
            base_delay_ms,
            config,
        }
    }

    /// Calculates the delay for a given retry attempt with jitter
    fn calculate_backoff_delay(&self, attempt: u32) -> Duration {
        // Exponential backoff: base_delay * 2^attempt
        let delay_ms = self.base_delay_ms.saturating_mul(1u64 << attempt);
        let capped_delay = delay_ms.min(MAX_DELAY_MS);

        // Add jitter (0-25% of the delay)
        let jitter = (capped_delay as f64 * 0.25 * rand_jitter()) as u64;
        Duration::from_millis(capped_delay + jitter)
    }

    /// Makes a single request attempt
    async fn make_request(
        &self,
        provider_req: &OpenAIChatRequest,
    ) -> Result<ChatResponse, (InferenceError, bool)> {
        let url = self.config.base_url.join("chat/completions").map_err(|e| {
            (
                InferenceError::ConfigError(format!("Invalid URL join: {}", e)),
                false, // Don't retry config errors
            )
        })?;

        let res = self
            .client
            .post(url)
            .header(
                "Authorization",
                format!("Bearer {}", self.config.api_key.expose_secret()),
            )
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
                let body: OpenAIChatResponse = res.json().await.map_err(|e| {
                    (
                        InferenceError::ProviderError(format!("Parse error: {}", e)),
                        false, // Don't retry parse errors
                    )
                })?;

                let choice = body.choices.first().ok_or_else(|| {
                    (
                        InferenceError::ProviderError("No choices returned".to_string()),
                        false,
                    )
                })?;

                Ok(ChatResponse {
                    content: choice.message.content.clone(),
                    usage: body.usage.map(|u| Usage {
                        prompt_tokens: u.prompt_tokens,
                        completion_tokens: u.completion_tokens,
                        total_tokens: u.total_tokens,
                    }),
                })
            }
            StatusCode::TOO_MANY_REQUESTS => {
                Err((InferenceError::RateLimit, true)) // Retry rate limits
            }
            StatusCode::BAD_REQUEST => {
                let text = res.text().await.unwrap_or_default();
                if text.contains("context_length_exceeded") {
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
                    true, // Retry server errors
                ))
            }
            _ => {
                let status = res.status();
                let text = res.text().await.unwrap_or_default();
                Err((
                    InferenceError::ProviderError(format!("HTTP {}: {}", status, text)),
                    false, // Don't retry other errors
                ))
            }
        }
    }
}

#[async_trait]
impl LLMProvider for OpenAIProvider {
    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse, InferenceError> {
        let provider_req = OpenAIChatRequest {
            model: request.model,
            messages: request.messages,
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
                        "Request failed, retrying after backoff"
                    );
                    tokio::time::sleep(delay).await;
                }
            }
        }

        debug!(
            attempts = self.max_retries + 1,
            "All retry attempts exhausted"
        );
        Err(last_error)
    }
}

/// Simple pseudo-random jitter between 0.0 and 1.0
/// Uses system time for simplicity (no external crate needed)
fn rand_jitter() -> f64 {
    use std::time::SystemTime;
    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    (nanos % 1000) as f64 / 1000.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use reqwest::Url;
    use secrecy::SecretString;

    #[test]
    fn test_openai_config_creation() {
        let api_key = SecretString::new("test-key".into());
        let base_url = Url::parse("https://api.openai.com/v1/").unwrap();
        let config = OpenAIConfig::new(api_key, base_url.clone());
        assert_eq!(config.base_url, base_url);
        assert!(config.max_retries.is_none());
        assert!(config.base_delay_ms.is_none());
    }

    #[test]
    fn test_openai_config_with_retries() {
        let api_key = SecretString::new("test-key".into());
        let base_url = Url::parse("https://api.openai.com/v1/").unwrap();
        let config = OpenAIConfig::new(api_key, base_url)
            .with_max_retries(5)
            .with_base_delay_ms(500);
        assert_eq!(config.max_retries, Some(5));
        assert_eq!(config.base_delay_ms, Some(500));
    }

    #[test]
    fn test_openai_provider_new() {
        let api_key = SecretString::new("test-key".into());
        let base_url = Url::parse("https://api.openai.com/v1/").unwrap();
        let config = OpenAIConfig::new(api_key, base_url);
        let provider = OpenAIProvider::new(config);
        assert_eq!(provider.max_retries, DEFAULT_MAX_RETRIES);
        assert_eq!(provider.base_delay_ms, DEFAULT_BASE_DELAY_MS);
    }

    #[test]
    fn test_openai_provider_with_custom_retries() {
        let api_key = SecretString::new("test-key".into());
        let base_url = Url::parse("https://api.openai.com/v1/").unwrap();
        let config = OpenAIConfig::new(api_key, base_url)
            .with_max_retries(10)
            .with_base_delay_ms(2000);
        let provider = OpenAIProvider::new(config);
        assert_eq!(provider.max_retries, 10);
        assert_eq!(provider.base_delay_ms, 2000);
    }

    #[test]
    fn test_backoff_delay_calculation() {
        let api_key = SecretString::new("test-key".into());
        let base_url = Url::parse("https://api.openai.com/v1/").unwrap();
        let config = OpenAIConfig::new(api_key, base_url).with_base_delay_ms(1000);
        let provider = OpenAIProvider::new(config);

        // First attempt: ~1000ms (+ jitter)
        let delay0 = provider.calculate_backoff_delay(0);
        assert!(delay0.as_millis() >= 1000);
        assert!(delay0.as_millis() <= 1250); // 1000 + 25% jitter

        // Second attempt: ~2000ms (+ jitter)
        let delay1 = provider.calculate_backoff_delay(1);
        assert!(delay1.as_millis() >= 2000);
        assert!(delay1.as_millis() <= 2500);
    }
}
