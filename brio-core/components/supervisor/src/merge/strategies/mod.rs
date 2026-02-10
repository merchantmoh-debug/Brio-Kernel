//! Merge Strategies - Trait and registry for merge strategies.
//!
//! This module defines the `MergeStrategy` trait and the `MergeStrategyRegistry`
//! for looking up strategies by name.

pub mod three_way;
pub mod union;

use async_trait::async_trait;
use std::collections::HashMap;
use std::path::Path;
use tracing::debug;

use crate::merge::conflict::{BranchResult, MAX_BRANCHES, MergeError, MergeResult};

/// Validates that the number of branches doesn't exceed the maximum allowed.
///
/// # Errors
/// Returns `MergeError::TooManyBranches` if the branch count exceeds the maximum.
pub fn validate_branch_count(branches: &[BranchResult]) -> Result<(), MergeError> {
    if branches.len() > MAX_BRANCHES {
        return Err(MergeError::TooManyBranches(branches.len()));
    }
    Ok(())
}

/// Trait for merge strategies.
#[async_trait]
pub trait MergeStrategy: Send + Sync {
    /// Returns the name of the strategy.
    fn name(&self) -> &'static str;
    /// Returns a description of the strategy.
    fn description(&self) -> &'static str;

    /// Merges changes from multiple branches.
    ///
    /// # Errors
    ///
    /// Returns an error if the merge cannot be completed due to I/O errors,
    /// diff computation failures, or if the strategy cannot handle the input.
    async fn merge(
        &self,
        base_path: &Path,
        branches: &[BranchResult],
    ) -> Result<MergeResult, MergeError>;
}

/// Registry for looking up merge strategies by name.
pub struct MergeStrategyRegistry {
    strategies: HashMap<String, Box<dyn MergeStrategy>>,
}

impl std::fmt::Debug for MergeStrategyRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MergeStrategyRegistry")
            .field("strategies", &self.strategies.keys().collect::<Vec<_>>())
            .finish()
    }
}

impl Default for MergeStrategyRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl MergeStrategyRegistry {
    /// Creates a new registry with default strategies registered.
    #[must_use]
    pub fn new() -> Self {
        use crate::merge::strategies::three_way::{OursStrategy, TheirsStrategy, ThreeWayStrategy};
        use crate::merge::strategies::union::UnionStrategy;

        let mut registry = Self {
            strategies: HashMap::new(),
        };
        registry.register(Box::new(OursStrategy));
        registry.register(Box::new(TheirsStrategy));
        registry.register(Box::new(UnionStrategy));
        registry.register(Box::new(ThreeWayStrategy::default()));
        registry
    }

    /// Registers a new strategy.
    pub fn register(&mut self, strategy: Box<dyn MergeStrategy>) {
        let name = strategy.name().to_string();
        debug!("Registering merge strategy: {}", name);
        self.strategies.insert(name, strategy);
    }

    /// Gets a strategy by name.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&dyn MergeStrategy> {
        self.strategies
            .get(name)
            .map(<Box<dyn MergeStrategy>>::as_ref)
    }

    /// Returns the default strategy (union).
    ///
    /// # Panics
    /// Panics if the union strategy is not registered (this should never happen).
    #[must_use]
    pub fn default_strategy(&self) -> &dyn MergeStrategy {
        self.get("union")
            .expect("Union strategy must always be registered")
    }

    /// Returns a list of all registered strategy names.
    #[must_use]
    pub fn available_strategies(&self) -> Vec<&str> {
        self.strategies.keys().map(String::as_str).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    
    

    #[test]
    fn test_registry_default_strategies() {
        let registry = MergeStrategyRegistry::new();

        assert!(registry.get("ours").is_some());
        assert!(registry.get("theirs").is_some());
        assert!(registry.get("union").is_some());
        assert!(registry.get("three-way").is_some());
    }

    #[test]
    fn test_registry_default_strategy_is_union() {
        let registry = MergeStrategyRegistry::new();

        assert_eq!(registry.default_strategy().name(), "union");
    }

    #[test]
    fn test_registry_available_strategies() {
        let registry = MergeStrategyRegistry::new();
        let strategies = registry.available_strategies();

        assert_eq!(strategies.len(), 4);
        assert!(strategies.contains(&"ours"));
        assert!(strategies.contains(&"theirs"));
        assert!(strategies.contains(&"union"));
        assert!(strategies.contains(&"three-way"));
    }
}
