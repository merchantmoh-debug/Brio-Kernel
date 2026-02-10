//! Branch management callbacks and types.
//!
//! This module provides types and callbacks for branch management tools
//! that allow agents to create and manage branches autonomously.
//! The callback-based approach avoids circular dependencies between
//! agent-sdk and supervisor.

use std::fmt;
use std::sync::Arc;

/// Represents a branch identifier.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BranchId(String);

impl BranchId {
    /// Creates a new `BranchId` from a string.
    #[must_use]
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Returns the inner string value.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for BranchId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Configuration for creating a new branch.
#[derive(Debug, Clone)]
pub struct BranchCreationConfig {
    /// Name of the branch.
    pub name: String,
    /// Base path or parent branch ID (optional).
    pub parent: Option<String>,
    /// Whether to inherit parent configuration.
    pub inherit_config: bool,
}

/// Result of a branch creation operation.
#[derive(Debug, Clone)]
pub struct BranchCreationResult {
    /// The ID of the created branch.
    pub branch_id: BranchId,
    /// The session ID for the branch workspace.
    pub session_id: String,
    /// Path to the branch workspace.
    pub workspace_path: String,
}

/// Error type for branch operations.
#[derive(Debug, Clone, thiserror::Error)]
pub enum BranchToolError {
    /// Maximum number of branches reached.
    #[error("Maximum number of concurrent branches reached")]
    MaxBranchesReached,
    /// Invalid branch name.
    #[error("Invalid branch name: {0}")]
    InvalidName(String),
    /// Parent branch not found.
    #[error("Parent branch not found: {0}")]
    ParentNotFound(String),
    /// Branch creation failed.
    #[error("Branch creation failed: {0}")]
    CreationFailed(String),
    /// Missing required argument.
    #[error("Missing required argument: {0}")]
    MissingArgument(String),
    /// Invalid argument value.
    #[error("Invalid value for argument '{name}': {value}")]
    InvalidArgument {
        /// Name of the invalid argument.
        name: String,
        /// The invalid value provided.
        value: String,
    },
}

/// Callback type for branch creation.
///
/// This allows the supervisor to provide the actual implementation
/// without creating a circular dependency.
pub type BranchCreationCallback = Arc<
    dyn Fn(BranchCreationConfig) -> Result<BranchCreationResult, BranchToolError> + Send + Sync,
>;

/// Information about a branch.
#[derive(Debug, Clone)]
pub struct BranchInfo {
    /// Branch identifier.
    pub id: BranchId,
    /// Branch name.
    pub name: String,
    /// Current status.
    pub status: String,
    /// Parent branch ID if applicable.
    pub parent_id: Option<String>,
    /// Creation timestamp.
    pub created_at: String,
}

/// Callback type for listing branches.
pub type BranchListCallback =
    Arc<dyn Fn() -> Result<Vec<BranchInfo>, BranchToolError> + Send + Sync>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_branch_id_creation() {
        let id = BranchId::new("test-branch-123");
        assert_eq!(id.as_str(), "test-branch-123");
    }
}
