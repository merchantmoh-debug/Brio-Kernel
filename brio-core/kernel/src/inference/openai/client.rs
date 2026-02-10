//! `OpenAI` API HTTP client implementation.
//!
//! This module provides the HTTP client for communicating with `OpenAI`'s API.

use crate::inference::openai::mapping::{
    OpenAIChatRequest, OpenAIChatResponse, create_request, map_response,
};
use crate::inference::openai::streaming::{DEFAULT_MAX_RETRIES, RetryConfig};
use crate::inference::provider::LLMProvider;
use crate::inference::types::{
    ChatRequest, ChatResponse, CircuitBreaker, CircuitBreakerConfig, InferenceError,
};
use anyhow::Result;
use async_trait::async_trait;
use reqwest::{Client, StatusCode, Url};
use secrecy::{ExposeSecret, SecretString};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, warn};

/// Configuration for the `OpenAI` provider
pub struct OpenAIConfig {
    /// The API key for authenticating with `OpenAI`
    pub api_key: SecretString,
    /// The base URL for the `OpenAI` API
    pub base_url: Url,
    /// Maximum number of retries for rate limits and transient errors
    pub max_retries: Option<u32>,
    /// Base delay in milliseconds for exponential backoff
    pub base_delay_ms: Option<u64>,
    /// Circuit breaker configuration for resilience
    pub circuit_breaker: Option<CircuitBreakerConfig>,
}

impl OpenAIConfig {
    /// Creates a new config with default retry settings
    #[must_use]
    pub fn new(api_key: SecretString, base_url: Url) -> Self {
        Self {
            api_key,
            base_url,
            max_retries: None,
            base_delay_ms: None,
            circuit_breaker: None,
        }
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

    /// Sets the circuit breaker configuration
    #[must_use]
    pub fn with_circuit_breaker(mut self, config: CircuitBreakerConfig) -> Self {
        self.circuit_breaker = Some(config);
        self
    }
}

/// Provider implementation for `OpenAI`'s API.
pub struct OpenAIProvider {
    client: Client,
    config: OpenAIConfig,
    retry_config: RetryConfig,
    circuit_breaker: Arc<RwLock<CircuitBreaker>>,
}

impl OpenAIProvider {
    /// Creates a new `OpenAI` provider with the given configuration.
    #[must_use]
    pub fn new(config: OpenAIConfig) -> Self {
        let max_retries = config.max_retries.unwrap_or(DEFAULT_MAX_RETRIES);
        let base_delay_ms = config.base_delay_ms.unwrap_or(1000);
        let cb_config = config.circuit_breaker.unwrap_or_default();

        Self {
            client: Client::new(),
            retry_config: RetryConfig::new()
                .with_max_retries(max_retries)
                .with_base_delay_ms(base_delay_ms),
            config,
            circuit_breaker: Arc::new(RwLock::new(CircuitBreaker::new(cb_config))),
        }
    }

    /// Makes a single request attempt
    async fn make_request(
        &self,
        provider_req: &OpenAIChatRequest,
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
        provider_req: &OpenAIChatRequest,
    ) -> Result<reqwest::RequestBuilder, InferenceError> {
        let url = self
            .config
            .base_url
            .join("chat/completions")
            .map_err(|e| InferenceError::ConfigError(format!("Invalid URL join: {e}")))?;

        Ok(self
            .client
            .post(url)
            .header(
                "Authorization",
                format!("Bearer {}", self.config.api_key.expose_secret()),
            )
            .header("Content-Type", "application/json")
            .json(provider_req))
    }

    async fn map_api_response(
        &self,
        res: reqwest::Response,
    ) -> Result<ChatResponse, (InferenceError, bool)> {
        match res.status() {
            StatusCode::OK => {
                let body: OpenAIChatResponse = res.json().await.map_err(|e| {
                    (
                        InferenceError::ProviderError(format!("Parse error: {e}")),
                        false,
                    )
                })?;

                map_response(body).map_err(|msg| (InferenceError::ProviderError(msg), false))
            }
            StatusCode::TOO_MANY_REQUESTS => Err((InferenceError::RateLimit, true)),
            StatusCode::BAD_REQUEST => {
                let text = res.text().await.unwrap_or_default();
                if text.contains("context_length_exceeded") {
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
impl LLMProvider for OpenAIProvider {
    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse, InferenceError> {
        // Check circuit breaker first
        let can_execute = {
            let mut cb = self.circuit_breaker.write().await;
            cb.try_acquire()
        };

        if !can_execute {
            return Err(InferenceError::CircuitBreakerOpen(
                "OpenAI circuit is open".to_string(),
            ));
        }

        let provider_req = create_request(request.model, request.messages);

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
                        "OpenAI request failed, retrying after backoff"
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
            "All OpenAI retry attempts exhausted"
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
    fn test_openai_config_creation() -> anyhow::Result<()> {
        let api_key = SecretString::new("test-key".into());
        let base_url = Url::parse("https://api.openai.com/v1/")?;
        let config = OpenAIConfig::new(api_key, base_url.clone());
        assert_eq!(config.base_url, base_url);
        assert!(config.max_retries.is_none());
        assert!(config.base_delay_ms.is_none());
        Ok(())
    }

    #[test]
    fn test_openai_config_with_retries() -> anyhow::Result<()> {
        let api_key = SecretString::new("test-key".into());
        let base_url = Url::parse("https://api.openai.com/v1/")?;
        let config = OpenAIConfig::new(api_key, base_url)
            .with_max_retries(5)
            .with_base_delay_ms(500);
        assert_eq!(config.max_retries, Some(5));
        assert_eq!(config.base_delay_ms, Some(500));
        Ok(())
    }

    #[test]
    fn test_openai_provider_new() -> anyhow::Result<()> {
        let api_key = SecretString::new("test-key".into());
        let base_url = Url::parse("https://api.openai.com/v1/")?;
        let config = OpenAIConfig::new(api_key, base_url);
        let provider = OpenAIProvider::new(config);
        assert_eq!(provider.retry_config.max_retries, DEFAULT_MAX_RETRIES);
        assert_eq!(provider.retry_config.base_delay_ms, 1000);
        Ok(())
    }

    #[test]
    fn test_openai_provider_with_custom_retries() -> anyhow::Result<()> {
        let api_key = SecretString::new("test-key".into());
        let base_url = Url::parse("https://api.openai.com/v1/")?;
        let config = OpenAIConfig::new(api_key, base_url)
            .with_max_retries(10)
            .with_base_delay_ms(2000);
        let provider = OpenAIProvider::new(config);
        assert_eq!(provider.retry_config.max_retries, 10);
        assert_eq!(provider.retry_config.base_delay_ms, 2000);
        Ok(())
    }
}
