//! Branch management tools for agents.
//!
//! This module provides tools that allow agents to create and manage branches
//! autonomously. The tools use a callback-based approach to avoid circular
//! dependencies between agent-sdk and supervisor.

use crate::error::ToolError;
use crate::tools::constants::branch;
use crate::tools::Tool;
use std::borrow::Cow;
use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

/// Represents a branch identifier.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BranchId(String);

impl BranchId {
    /// Creates a new BranchId from a string.
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

/// Tool for creating a new branch.
///
/// This tool allows agents to create isolated workspaces (branches)
/// for parallel or speculative work. The tool delegates the actual
/// branch creation to a callback provided by the supervisor.
///
/// # Example
///
/// ```rust,no_run
/// use agent_sdk::agent::tools::{CreateBranchTool, BranchCreationCallback};
/// use agent_sdk::Tool;
/// use std::sync::Arc;
/// use std::collections::HashMap;
///
/// # fn example(callback: BranchCreationCallback) {
/// let tool = CreateBranchTool::new(callback);
/// # }
/// ```
pub struct CreateBranchTool {
    creation_callback: BranchCreationCallback,
}

impl CreateBranchTool {
    /// Creates a new `CreateBranchTool` with the specified creation callback.
    ///
    /// # Arguments
    ///
    /// * `creation_callback` - Function that performs the actual branch creation
    #[must_use]
    pub fn new(creation_callback: BranchCreationCallback) -> Self {
        Self { creation_callback }
    }
}

/// Parses branch creation configuration from tool arguments.
///
/// # Errors
///
/// Returns `BranchToolError` if required arguments are missing or invalid.
fn parse_branch_config(
    args: &HashMap<String, String>,
) -> Result<BranchCreationConfig, BranchToolError> {
    let name = args
        .get("name")
        .ok_or_else(|| BranchToolError::MissingArgument("name".to_string()))?
        .clone();

    // Validate branch name
    if name.is_empty() {
        return Err(BranchToolError::InvalidName(
            "Branch name cannot be empty".to_string(),
        ));
    }

    if name.len() > 256 {
        return Err(BranchToolError::InvalidName(
            "Branch name exceeds 256 characters".to_string(),
        ));
    }

    let parent = args.get("parent").cloned();
    let inherit_config = args
        .get("inherit_config")
        .map(|v| v.parse::<bool>().unwrap_or(true))
        .unwrap_or(true);

    Ok(BranchCreationConfig {
        name,
        parent,
        inherit_config,
    })
}

impl Tool for CreateBranchTool {
    fn name(&self) -> Cow<'static, str> {
        Cow::Borrowed(branch::CREATE_BRANCH)
    }

    fn description(&self) -> Cow<'static, str> {
        Cow::Borrowed(
            r#"<create_branch name="branch-name" [parent="parent-id"] [inherit_config="true"]/>

Creates a new isolated workspace (branch) for parallel or speculative work.

Arguments:
- name (required): Unique name for the branch (1-256 characters)
- parent (optional): Parent branch ID to branch from. If not specified, creates from base.
- inherit_config (optional): Whether to inherit parent configuration (default: true)

Returns:
- branch_id: Unique identifier for the created branch
- session_id: Workspace session identifier
- workspace_path: Path to the branch workspace

Example:
<create_branch name="feature-auth" parent="main-branch" />

Limitations:
- Maximum 8 concurrent branches system-wide
- Branch names must be unique within parent scope"#,
        )
    }

    fn execute(&self, args: &HashMap<String, String>) -> Result<String, ToolError> {
        // Parse configuration
        let config = parse_branch_config(args).map_err(|e| ToolError::ExecutionFailed {
            tool: branch::CREATE_BRANCH.to_string(),
            source: Box::new(e),
        })?;

        // Execute branch creation via callback
        let result = (self.creation_callback)(config).map_err(|e| ToolError::ExecutionFailed {
            tool: branch::CREATE_BRANCH.to_string(),
            source: Box::new(e),
        })?;

        // Format successful result
        let output = format!(
            "Branch created successfully:\n\
             - Branch ID: {}\n\
             - Session ID: {}\n\
             - Workspace: {}",
            result.branch_id, result.session_id, result.workspace_path
        );

        Ok(output)
    }
}

/// Tool for listing existing branches.
///
/// Allows agents to query available branches and their status.
pub struct ListBranchesTool {
    list_callback: Arc<dyn Fn() -> Result<Vec<BranchInfo>, BranchToolError> + Send + Sync>,
}

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

impl ListBranchesTool {
    /// Creates a new `ListBranchesTool` with the specified list callback.
    #[must_use]
    pub fn new(
        list_callback: Arc<dyn Fn() -> Result<Vec<BranchInfo>, BranchToolError> + Send + Sync>,
    ) -> Self {
        Self { list_callback }
    }
}

impl Tool for ListBranchesTool {
    fn name(&self) -> Cow<'static, str> {
        Cow::Borrowed(branch::LIST_BRANCHES)
    }

    fn description(&self) -> Cow<'static, str> {
        Cow::Borrowed(
            r#"<list_branches />

Lists all active branches in the system.

Returns:
- List of branches with their ID, name, status, and creation time

Example:
<list_branches />"#,
        )
    }

    fn execute(&self, _args: &HashMap<String, String>) -> Result<String, ToolError> {
        let branches = (self.list_callback)().map_err(|e| ToolError::ExecutionFailed {
            tool: branch::LIST_BRANCHES.to_string(),
            source: Box::new(e),
        })?;

        if branches.is_empty() {
            return Ok("No active branches found.".to_string());
        }

        // Pre-allocate with estimated capacity (avg 50 chars per branch + header)
        let estimated_capacity = branches.len() * 50 + 20;
        let mut output = String::with_capacity(estimated_capacity);
        output.push_str("Active branches:\n");
        for branch in branches {
            output.push_str(&format!(
                "- {} ({}): {} - {}\n",
                branch.name, branch.id, branch.status, branch.created_at
            ));
        }

        Ok(output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_branch_id_creation() {
        let id = BranchId::new("test-branch-123");
        assert_eq!(id.as_str(), "test-branch-123");
    }

    #[test]
    fn test_create_branch_tool_parsing() {
        let callback: BranchCreationCallback = Arc::new(|config| {
            Ok(BranchCreationResult {
                branch_id: BranchId::new("test-id"),
                session_id: "session-123".to_string(),
                workspace_path: "/workspace/test".to_string(),
            })
        });

        let tool = CreateBranchTool::new(callback);
        let mut args = HashMap::new();
        args.insert("name".to_string(), "test-branch".to_string());

        let result = tool.execute(&args);
        assert!(result.is_ok());
        assert!(result.unwrap().contains("test-id"));
    }

    #[test]
    fn test_create_branch_missing_name() {
        let callback: BranchCreationCallback = Arc::new(|_| {
            Ok(BranchCreationResult {
                branch_id: BranchId::new("test-id"),
                session_id: "session-123".to_string(),
                workspace_path: "/workspace/test".to_string(),
            })
        });

        let tool = CreateBranchTool::new(callback);
        let args = HashMap::new();

        let result = tool.execute(&args);
        assert!(result.is_err());
    }

    #[test]
    fn test_create_branch_empty_name() {
        let callback: BranchCreationCallback = Arc::new(|_| {
            Ok(BranchCreationResult {
                branch_id: BranchId::new("test-id"),
                session_id: "session-123".to_string(),
                workspace_path: "/workspace/test".to_string(),
            })
        });

        let tool = CreateBranchTool::new(callback);
        let mut args = HashMap::new();
        args.insert("name".to_string(), "".to_string());

        let result = tool.execute(&args);
        assert!(result.is_err());
    }
}
