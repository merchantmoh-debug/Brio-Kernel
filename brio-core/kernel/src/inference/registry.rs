use crate::inference::provider::LLMProvider;
use crate::inference::types::{ChatRequest, ChatResponse, InferenceError};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use tracing::debug;

/// A registry for managing multiple LLM providers.
///
/// Allows routing requests to different providers by name, enabling
/// concurrent use of multiple LLM backends (OpenAI, Anthropic, etc.).
pub struct ProviderRegistry {
    providers: RwLock<HashMap<String, Arc<dyn LLMProvider>>>,
    default_provider: RwLock<Option<String>>,
}

impl ProviderRegistry {
    /// Creates a new empty registry
    pub fn new() -> Self {
        Self {
            providers: RwLock::new(HashMap::new()),
            default_provider: RwLock::new(None),
        }
    }

    /// Registers a provider with the given name
    pub fn register(&self, name: impl Into<String>, provider: impl LLMProvider + 'static) {
        let name = name.into();
        debug!(provider_name = %name, "Registering LLM provider");
        let mut providers = self.providers.write().expect("RwLock poisoned");
        providers.insert(name, Arc::new(provider));
    }

    /// Registers a provider wrapped in Arc
    pub fn register_arc(&self, name: impl Into<String>, provider: Arc<dyn LLMProvider>) {
        let name = name.into();
        debug!(provider_name = %name, "Registering LLM provider (Arc)");
        let mut providers = self.providers.write().expect("RwLock poisoned");
        providers.insert(name, provider);
    }

    /// Sets the default provider name
    pub fn set_default(&self, name: impl Into<String>) {
        let name = name.into();
        debug!(provider_name = %name, "Setting default LLM provider");
        let mut default = self.default_provider.write().expect("RwLock poisoned");
        *default = Some(name);
    }

    /// Gets a provider by name
    pub fn get(&self, name: &str) -> Option<Arc<dyn LLMProvider>> {
        let providers = self.providers.read().expect("RwLock poisoned");
        providers.get(name).cloned()
    }

    /// Gets the default provider
    pub fn get_default(&self) -> Option<Arc<dyn LLMProvider>> {
        let default_name = {
            let default = self.default_provider.read().expect("RwLock poisoned");
            default.clone()
        };

        match default_name {
            Some(name) => self.get(&name),
            None => {
                // If no default set, return first registered provider
                let providers = self.providers.read().expect("RwLock poisoned");
                providers.values().next().cloned()
            }
        }
    }

    /// Lists all registered provider names
    pub fn list_providers(&self) -> Vec<String> {
        let providers = self.providers.read().expect("RwLock poisoned");
        providers.keys().cloned().collect()
    }

    /// Returns the number of registered providers
    pub fn len(&self) -> usize {
        let providers = self.providers.read().expect("RwLock poisoned");
        providers.len()
    }

    /// Returns true if no providers are registered
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Removes a provider by name
    pub fn remove(&self, name: &str) -> Option<Arc<dyn LLMProvider>> {
        debug!(provider_name = %name, "Removing LLM provider");
        let mut providers = self.providers.write().expect("RwLock poisoned");
        providers.remove(name)
    }

    /// Sends a chat request to the named provider
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
    pub async fn chat_default(&self, request: ChatRequest) -> Result<ChatResponse, InferenceError> {
        let provider = self.get_default().ok_or_else(|| {
            InferenceError::ProviderNotFound("No default provider configured".to_string())
        })?;

        provider.chat(request).await
    }
}

impl Default for ProviderRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inference::types::{Message, Role};
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

    #[test]
    fn test_registry_new() {
        let registry = ProviderRegistry::new();
        assert!(registry.is_empty());
        assert_eq!(registry.len(), 0);
    }

    #[test]
    fn test_registry_register_and_get() {
        let registry = ProviderRegistry::new();
        registry.register(
            "openai",
            MockProvider {
                response: "OpenAI response".to_string(),
            },
        );

        assert!(!registry.is_empty());
        assert_eq!(registry.len(), 1);
        assert!(registry.get("openai").is_some());
        assert!(registry.get("anthropic").is_none());
    }

    #[test]
    fn test_registry_multiple_providers() {
        let registry = ProviderRegistry::new();
        registry.register(
            "openai",
            MockProvider {
                response: "OpenAI".to_string(),
            },
        );
        registry.register(
            "anthropic",
            MockProvider {
                response: "Anthropic".to_string(),
            },
        );

        assert_eq!(registry.len(), 2);
        assert!(registry.get("openai").is_some());
        assert!(registry.get("anthropic").is_some());
    }

    #[test]
    fn test_registry_list_providers() {
        let registry = ProviderRegistry::new();
        registry.register(
            "openai",
            MockProvider {
                response: "test".to_string(),
            },
        );
        registry.register(
            "anthropic",
            MockProvider {
                response: "test".to_string(),
            },
        );

        let providers = registry.list_providers();
        assert_eq!(providers.len(), 2);
        assert!(providers.contains(&"openai".to_string()));
        assert!(providers.contains(&"anthropic".to_string()));
    }

    #[test]
    fn test_registry_default_provider() {
        let registry = ProviderRegistry::new();
        registry.register(
            "openai",
            MockProvider {
                response: "OpenAI".to_string(),
            },
        );
        registry.register(
            "anthropic",
            MockProvider {
                response: "Anthropic".to_string(),
            },
        );
        registry.set_default("anthropic");

        assert!(registry.get_default().is_some());
    }

    #[test]
    fn test_registry_remove() {
        let registry = ProviderRegistry::new();
        registry.register(
            "openai",
            MockProvider {
                response: "test".to_string(),
            },
        );

        assert!(registry.get("openai").is_some());
        let removed = registry.remove("openai");
        assert!(removed.is_some());
        assert!(registry.get("openai").is_none());
        assert!(registry.is_empty());
    }

    #[tokio::test]
    async fn test_registry_chat() {
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
    async fn test_registry_chat_provider_not_found() {
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
    async fn test_registry_chat_default() {
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
}
