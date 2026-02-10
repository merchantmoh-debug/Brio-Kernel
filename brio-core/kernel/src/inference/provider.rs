//! LLM provider trait definition.
//!
//! This module defines the common interface for all LLM providers.

use crate::inference::types::{ChatRequest, ChatResponse, InferenceError};
use async_trait::async_trait;
use std::sync::Arc;
use tracing::{debug, info, warn};

/// A trait for LLM providers that can execute chat completion requests.
#[async_trait]
pub trait LLMProvider: Send + Sync {
    /// Executes a chat completion request.
    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse, InferenceError>;
}

/// A provider that chains multiple LLM providers as fallbacks.
///
/// Attempts each provider in order until one succeeds or all fail.
/// Only retries on retryable errors (transient failures, rate limits).
#[derive(Clone)]
pub struct FallbackProviderChain {
    providers: Vec<Arc<dyn LLMProvider>>,
    provider_names: Vec<String>,
}

impl FallbackProviderChain {
    /// Creates a new fallback provider chain.
    #[must_use]
    pub fn new() -> Self {
        Self {
            providers: Vec::new(),
            provider_names: Vec::new(),
        }
    }

    /// Adds a provider to the chain.
    #[must_use]
    pub fn add_provider<P: LLMProvider + 'static>(
        mut self,
        name: impl Into<String>,
        provider: P,
    ) -> Self {
        self.providers.push(Arc::new(provider));
        self.provider_names.push(name.into());
        self
    }

    /// Returns the number of providers in the chain.
    #[must_use]
    pub fn len(&self) -> usize {
        self.providers.len()
    }

    /// Returns `true` if the chain has no providers.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.providers.is_empty()
    }

    /// Returns the names of all providers in the chain.
    #[must_use]
    pub fn provider_names(&self) -> &[String] {
        &self.provider_names
    }
}

impl Default for FallbackProviderChain {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl LLMProvider for FallbackProviderChain {
    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse, InferenceError> {
        if self.providers.is_empty() {
            return Err(InferenceError::ProviderNotFound(
                "Fallback chain is empty".to_string(),
            ));
        }

        for (idx, (provider, name)) in self
            .providers
            .iter()
            .zip(self.provider_names.iter())
            .enumerate()
        {
            debug!(provider = %name, index = idx, "Attempting provider in fallback chain");

            match provider.chat(request.clone()).await {
                Ok(response) => {
                    info!(provider = %name, "Provider succeeded in fallback chain");
                    return Ok(response);
                }
                Err(err) => {
                    warn!(
                        provider = %name,
                        error = %err,
                        retryable = err.is_retryable(),
                        "Provider failed in fallback chain"
                    );

                    // Only continue to next provider if this was a retryable error
                    // Permanent errors (like invalid config) should not trigger fallback
                    if !err.is_retryable() && !err.is_circuit_breaker() {
                        debug!(
                            provider = %name,
                            "Non-retryable error, stopping fallback chain"
                        );
                        break;
                    }
                }
            }
        }

        warn!("All providers in fallback chain failed");
        Err(InferenceError::AllProvidersFailed)
    }
}
