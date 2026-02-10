//! Provider registry core implementation.
//!
//! This module provides the [`ProviderRegistry`] which allows concurrent
//! registration and use of multiple LLM providers.

use crate::inference::provider::LLMProvider;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::debug;

/// A registry for managing multiple LLM providers.
///
/// Allows routing requests to different providers by name, enabling
/// concurrent use of multiple LLM backends (`OpenAI`, Anthropic, etc.).
pub struct ProviderRegistry {
    pub(crate) providers: RwLock<HashMap<String, Arc<dyn LLMProvider>>>,
    pub(crate) default_provider: RwLock<Option<String>>,
}

impl ProviderRegistry {
    /// Creates a new empty registry
    #[must_use]
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
        let mut providers = self.providers.write();
        providers.insert(name, Arc::new(provider));
    }

    /// Registers a provider wrapped in Arc
    pub fn register_arc(&self, name: impl Into<String>, provider: Arc<dyn LLMProvider>) {
        let name = name.into();
        debug!(provider_name = %name, "Registering LLM provider (Arc)");
        let mut providers = self.providers.write();
        providers.insert(name, provider);
    }

    /// Sets the default provider name
    pub fn set_default(&self, name: impl Into<String>) {
        let name = name.into();
        debug!(provider_name = %name, "Setting default LLM provider");
        let mut default = self.default_provider.write();
        *default = Some(name);
    }

    /// Gets a provider by name
    pub fn get(&self, name: &str) -> Option<Arc<dyn LLMProvider>> {
        let providers = self.providers.read();
        providers.get(name).cloned()
    }

    /// Gets the default provider
    pub fn default_provider(&self) -> Option<Arc<dyn LLMProvider>> {
        let default_name = {
            let default = self.default_provider.read();
            default.clone()
        };

        if let Some(name) = default_name {
            self.get(&name)
        } else {
            // If no default set, return first registered provider
            let providers = self.providers.read();
            providers.values().next().cloned()
        }
    }

    /// Lists all registered provider names
    pub fn list_providers(&self) -> Vec<String> {
        let providers = self.providers.read();
        providers.keys().cloned().collect()
    }

    /// Returns the number of registered providers
    pub fn len(&self) -> usize {
        let providers = self.providers.read();
        providers.len()
    }

    /// Returns true if no providers are registered
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Removes a provider by name
    pub fn remove(&self, name: &str) -> Option<Arc<dyn LLMProvider>> {
        debug!(provider_name = %name, "Removing LLM provider");
        let mut providers = self.providers.write();
        providers.remove(name)
    }
}

impl Default for ProviderRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inference::types::{ChatRequest, ChatResponse, InferenceError};
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
    fn registry_should_create_empty() {
        let registry = ProviderRegistry::new();
        assert!(registry.is_empty());
        assert_eq!(registry.len(), 0);
    }

    #[test]
    fn register_should_add_provider_and_make_it_retrievable() {
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
    fn registry_should_support_multiple_providers() {
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
    fn list_providers_should_return_all_registered_names() {
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
    fn set_default_should_make_provider_retrievable_as_default() {
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

        assert!(registry.default_provider().is_some());
    }

    #[test]
    fn remove_should_delete_provider_from_registry() {
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

    #[test]
    fn default_returns_first_when_no_explicit_default() {
        let registry = ProviderRegistry::new();
        registry.register(
            "openai",
            MockProvider {
                response: "OpenAI".to_string(),
            },
        );

        // No explicit default set, should return first registered
        let default = registry.default_provider();
        assert!(default.is_some());
    }

    #[test]
    fn register_arc_works_correctly() {
        let registry = ProviderRegistry::new();
        let provider = Arc::new(MockProvider {
            response: "Arc provider".to_string(),
        });

        registry.register_arc("test", provider);
        assert_eq!(registry.len(), 1);
        assert!(registry.get("test").is_some());
    }
}
