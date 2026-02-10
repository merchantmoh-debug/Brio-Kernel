//! Anthropic API HTTP client implementation.
//!
//! This module provides the HTTP client for communicating with Anthropic's API.

use crate::inference::anthropic::mapping::{AnthropicChatRequest, map_response, prepare_messages};
use crate::inference::anthropic::retry::{DEFAULT_MAX_RETRIES, RetryConfig};
use crate::inference::provider::LLMProvider;
use crate::inference::types::{
    ChatRequest, ChatResponse, CircuitBreaker, CircuitBreakerConfig, InferenceError,
};
use async_trait::async_trait;
use reqwest::{Client, StatusCode, Url};
use secrecy::{ExposeSecret, SecretString};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, warn};

/// Default Anthropic API version
const DEFAULT_API_VERSION: &str = "2023-06-01";

/// Configuration for the Anthropic provider
pub struct AnthropicConfig {
    /// The API key for authenticating with Anthropic
    pub api_key: SecretString,
    /// The base URL for the Anthropic API
    pub base_url: Url,
    /// Maximum number of retries for rate limits and transient errors
    pub max_retries: Option<u32>,
    /// Base delay in milliseconds for exponential backoff
    pub base_delay_ms: Option<u64>,
    /// API version header value
    pub api_version: Option<String>,
    /// Maximum tokens to generate (required by Anthropic API)
    pub max_tokens: Option<u32>,
    /// Circuit breaker configuration for resilience
    pub circuit_breaker: Option<CircuitBreakerConfig>,
}

impl AnthropicConfig {
    /// Creates a new config with default settings
    #[must_use]
    pub fn new(api_key: SecretString, base_url: Url) -> Self {
        Self {
            api_key,
            base_url,
            max_retries: None,
            base_delay_ms: None,
            api_version: None,
            max_tokens: None,
            circuit_breaker: None,
        }
    }

    /// Creates a config with default Anthropic base URL
    ///
    /// # Errors
    ///
    /// Returns an error if the hardcoded URL is invalid.
    pub fn with_api_key(api_key: SecretString) -> anyhow::Result<Self> {
        Ok(Self::new(
            api_key,
            Url::parse("https://api.anthropic.com/v1/")
                .map_err(|e| anyhow::anyhow!("Invalid hardcoded URL: {e}"))?,
        ))
    }

    /// Sets the circuit breaker configuration
    #[must_use]
    pub fn with_circuit_breaker(mut self, config: CircuitBreakerConfig) -> Self {
        self.circuit_breaker = Some(config);
        self
    }

    /// Sets the maximum number of retries
    #[must_use]
    pub fn with_max_retries(mut self, max_retries: u32) -> Self {
        self.max_retries = Some(max_retries);
        self
    }

    /// Sets the base delay for exponential backoff
    #[must_use]
    pub fn with_base_delay_ms(mut self, delay_ms: u64) -> Self {
        self.base_delay_ms = Some(delay_ms);
        self
    }

    /// Sets the API version
    #[must_use]
    pub fn with_api_version(mut self, version: String) -> Self {
        self.api_version = Some(version);
        self
    }

    /// Sets the maximum tokens to generate
    #[must_use]
    pub fn with_max_tokens(mut self, max_tokens: u32) -> Self {
        self.max_tokens = Some(max_tokens);
        self
    }
}

/// Provider implementation for Anthropic's Claude API.
pub struct AnthropicProvider {
    client: Client,
    config: AnthropicConfig,
    retry_config: RetryConfig,
    api_version: String,
    max_tokens: u32,
    circuit_breaker: Arc<RwLock<CircuitBreaker>>,
}

impl AnthropicProvider {
    /// Creates a new Anthropic provider with the given configuration.
    #[must_use]
    pub fn new(config: AnthropicConfig) -> Self {
        let max_retries = config.max_retries.unwrap_or(DEFAULT_MAX_RETRIES);
        let base_delay_ms = config.base_delay_ms.unwrap_or(1000);
        let api_version = config
            .api_version
            .clone()
            .unwrap_or_else(|| DEFAULT_API_VERSION.to_string());
        let max_tokens = config.max_tokens.unwrap_or(4096);
        let cb_config = config.circuit_breaker.unwrap_or_default();

        Self {
            client: Client::new(),
            retry_config: RetryConfig::new()
                .with_max_retries(max_retries)
                .with_base_delay_ms(base_delay_ms),
            api_version,
            max_tokens,
            config,
            circuit_breaker: Arc::new(RwLock::new(CircuitBreaker::new(cb_config))),
        }
    }

    /// Makes a single request attempt
    async fn make_request(
        &self,
        provider_req: &AnthropicChatRequest,
    ) -> Result<ChatResponse, (InferenceError, bool)> {
        let request = self
            .build_api_request(provider_req)
            .map_err(|e| (e, false))?;

        let res = request.send().await.map_err(|e| {
            (
                InferenceError::NetworkError(e.to_string()),
                true, // Retry network errors
            )
        })?;

        self.map_api_response(res).await
    }

    fn build_api_request(
        &self,
        provider_req: &AnthropicChatRequest,
    ) -> Result<reqwest::RequestBuilder, InferenceError> {
        let url = self
            .config
            .base_url
            .join("messages")
            .map_err(|e| InferenceError::ConfigError(format!("Invalid URL join: {e}")))?;

        Ok(self
            .client
            .post(url)
            .header("x-api-key", self.config.api_key.expose_secret())
            .header("anthropic-version", &self.api_version)
            .header("Content-Type", "application/json")
            .json(provider_req))
    }

    async fn map_api_response(
        &self,
        res: reqwest::Response,
    ) -> Result<ChatResponse, (InferenceError, bool)> {
        match res.status() {
            StatusCode::OK => {
                let body = res.json().await.map_err(|e| {
                    (
                        InferenceError::ProviderError(format!("Parse error: {e}")),
                        false,
                    )
                })?;

                Ok(map_response(body))
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
                        InferenceError::ProviderError(format!("Bad Request: {text}")),
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
                    InferenceError::ProviderError(format!("HTTP {status}: {text}")),
                    true,
                ))
            }
            _ => {
                let status = res.status();
                let text = res.text().await.unwrap_or_default();
                Err((
                    InferenceError::ProviderError(format!("HTTP {status}: {text}")),
                    false,
                ))
            }
        }
    }
}

#[async_trait]
impl LLMProvider for AnthropicProvider {
    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse, InferenceError> {
        // Check circuit breaker first
        let can_execute = {
            let mut cb = self.circuit_breaker.write().await;
            cb.try_acquire()
        };

        if !can_execute {
            return Err(InferenceError::CircuitBreakerOpen(
                "Anthropic circuit is open".to_string(),
            ));
        }

        let (system, messages) = prepare_messages(&request.messages);

        let provider_req = AnthropicChatRequest {
            model: request.model,
            max_tokens: self.max_tokens,
            messages,
            system,
        };

        let mut last_error = InferenceError::NetworkError("No attempts made".to_string());

        for attempt in 0..=self.retry_config.max_retries {
            match self.make_request(&provider_req).await {
                Ok(response) => {
                    // Record success on circuit breaker
                    let mut cb = self.circuit_breaker.write().await;
                    cb.record_success();
                    return Ok(response);
                }
                Err((error, should_retry)) => {
                    last_error = error;

                    if !should_retry || attempt >= self.retry_config.max_retries {
                        break;
                    }

                    let delay = self.retry_config.calculate_backoff_delay(attempt);
                    let delay_ms: u64 = delay.as_millis().try_into().unwrap_or(u64::MAX);
                    warn!(
                        attempt = attempt + 1,
                        max_retries = self.retry_config.max_retries,
                        delay_ms = delay_ms,
                        error = %last_error,
                        "Anthropic request failed, retrying after backoff"
                    );
                    tokio::time::sleep(delay).await;
                }
            }
        }

        // Record failure on circuit breaker
        let mut cb = self.circuit_breaker.write().await;
        cb.record_failure();

        debug!(
            attempts = self.retry_config.max_retries + 1,
            "All Anthropic retry attempts exhausted"
        );
        Err(last_error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use reqwest::Url;
    use secrecy::SecretString;

    #[test]
    fn test_anthropic_config_creation() -> anyhow::Result<()> {
        let api_key = SecretString::new("test-key".into());
        let base_url = Url::parse("https://api.anthropic.com/v1/")?;
        let config = AnthropicConfig::new(api_key, base_url.clone());
        assert_eq!(config.base_url, base_url);
        assert!(config.max_retries.is_none());
        assert!(config.base_delay_ms.is_none());
        Ok(())
    }

    #[test]
    fn test_anthropic_config_with_api_key() -> anyhow::Result<()> {
        let api_key = SecretString::new("test-key".into());
        let config = AnthropicConfig::with_api_key(api_key)?;
        assert_eq!(config.base_url.as_str(), "https://api.anthropic.com/v1/");
        Ok(())
    }

    #[test]
    fn test_anthropic_config_with_retries() -> anyhow::Result<()> {
        let api_key = SecretString::new("test-key".into());
        let base_url = Url::parse("https://api.anthropic.com/v1/")?;
        let config = AnthropicConfig::new(api_key, base_url)
            .with_max_retries(5)
            .with_base_delay_ms(500)
            .with_max_tokens(8192);
        assert_eq!(config.max_retries, Some(5));
        assert_eq!(config.base_delay_ms, Some(500));
        assert_eq!(config.max_tokens, Some(8192));
        Ok(())
    }

    #[test]
    fn test_anthropic_provider_new() -> anyhow::Result<()> {
        let api_key = SecretString::new("test-key".into());
        let base_url = Url::parse("https://api.anthropic.com/v1/")?;
        let config = AnthropicConfig::new(api_key, base_url);
        let provider = AnthropicProvider::new(config);
        assert_eq!(provider.retry_config.max_retries, DEFAULT_MAX_RETRIES);
        assert_eq!(provider.retry_config.base_delay_ms, 1000);
        assert_eq!(provider.api_version, DEFAULT_API_VERSION);
        Ok(())
    }
}
