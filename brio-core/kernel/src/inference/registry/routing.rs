//! Provider routing implementation.
//!
//! This module provides request routing to registered providers.

use crate::inference::registry::core::ProviderRegistry;
use crate::inference::types::{ChatRequest, ChatResponse, InferenceError};

impl ProviderRegistry {
    /// Sends a chat request to the named provider
    ///
    /// # Errors
    ///
    /// Returns an error if the provider is not found or if the chat request fails.
    pub async fn chat(
        &self,
        provider_name: &str,
        request: ChatRequest,
    ) -> Result<ChatResponse, InferenceError> {
        let provider = self
            .get(provider_name)
            .ok_or_else(|| InferenceError::ProviderNotFound(provider_name.to_string()))?;

        provider.chat(request).await
    }

    /// Sends a chat request to the default provider
    ///
    /// # Errors
    ///
    /// Returns an error if no default provider is configured or if the chat request fails.
    pub async fn chat_default(&self, request: ChatRequest) -> Result<ChatResponse, InferenceError> {
        let provider = self.default_provider().ok_or_else(|| {
            InferenceError::ProviderNotFound("No default provider configured".to_string())
        })?;

        provider.chat(request).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inference::provider::LLMProvider;
    use crate::inference::types::{ChatRequest, ChatResponse, InferenceError, Message, Role};
    use async_trait::async_trait;

    struct MockProvider {
        response: String,
    }

    #[async_trait]
    impl LLMProvider for MockProvider {
        async fn chat(&self, _request: ChatRequest) -> Result<ChatResponse, InferenceError> {
            Ok(ChatResponse {
                content: self.response.clone(),
                usage: None,
            })
        }
    }

    #[tokio::test]
    async fn chat_should_return_response_from_named_provider() {
        let registry = ProviderRegistry::new();
        registry.register(
            "openai",
            MockProvider {
                response: "Hello from OpenAI".to_string(),
            },
        );

        let request = ChatRequest {
            model: "gpt-4".to_string(),
            messages: vec![Message {
                role: Role::User,
                content: "Hi".to_string(),
            }],
        };

        let response = registry.chat("openai", request).await.unwrap();
        assert_eq!(response.content, "Hello from OpenAI");
    }

    #[tokio::test]
    async fn chat_should_return_error_when_provider_not_found() {
        let registry = ProviderRegistry::new();

        let request = ChatRequest {
            model: "gpt-4".to_string(),
            messages: vec![],
        };

        let result = registry.chat("nonexistent", request).await;
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            InferenceError::ProviderNotFound(_)
        ));
    }

    #[tokio::test]
    async fn chat_default_should_use_default_provider() {
        let registry = ProviderRegistry::new();
        registry.register(
            "anthropic",
            MockProvider {
                response: "Anthropic response".to_string(),
            },
        );
        registry.set_default("anthropic");

        let request = ChatRequest {
            model: "claude-3".to_string(),
            messages: vec![Message {
                role: Role::User,
                content: "Hello".to_string(),
            }],
        };

        let response = registry.chat_default(request).await.unwrap();
        assert_eq!(response.content, "Anthropic response");
    }

    #[tokio::test]
    async fn chat_default_should_error_when_no_default() {
        let registry = ProviderRegistry::new();
        // No providers registered at all

        let request = ChatRequest {
            model: "gpt-4".to_string(),
            messages: vec![],
        };

        let result = registry.chat_default(request).await;
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            InferenceError::ProviderNotFound(_)
        ));
    }

    #[tokio::test]
    async fn chat_default_uses_first_when_no_explicit_default() {
        let registry = ProviderRegistry::new();
        registry.register(
            "openai",
            MockProvider {
                response: "OpenAI default".to_string(),
            },
        );

        let request = ChatRequest {
            model: "gpt-4".to_string(),
            messages: vec![Message {
                role: Role::User,
                content: "Hello".to_string(),
            }],
        };

        let response = registry.chat_default(request).await.unwrap();
        assert_eq!(response.content, "OpenAI default");
    }
}
