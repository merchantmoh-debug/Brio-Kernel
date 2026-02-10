//! Request/Response Types for Branch API
//!
//! This module provides DTOs for branch management operations.

use crate::branch_manager::Branch;
use serde::{Deserialize, Serialize};

/// Request to create a new branch.
#[derive(Debug, Clone, Deserialize)]
pub struct CreateBranchRequest {
    /// Source of the branch (base directory or parent branch).
    pub source: BranchSourceRequest,
    /// Branch configuration.
    pub config: BranchConfigRequest,
}

/// Source specification for branch creation.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type")]
pub enum BranchSourceRequest {
    /// Create from a base filesystem path.
    #[serde(rename = "base")]
    Base {
        /// Path to the base directory.
        path: String,
    },
    /// Create from an existing branch.
    #[serde(rename = "branch")]
    Branch {
        /// ID of the parent branch.
        branch_id: String,
    },
}

/// Branch configuration request.
#[derive(Debug, Clone, Deserialize)]
pub struct BranchConfigRequest {
    /// Human-readable branch name.
    pub name: String,
    /// Agent assignments for this branch.
    pub agents: Vec<AgentAssignmentRequest>,
    /// Execution strategy (sequential or parallel).
    #[serde(default)]
    pub execution_strategy: ExecutionStrategyRequest,
    /// Whether to auto-merge on completion.
    #[serde(default)]
    pub auto_merge: bool,
    /// Merge strategy to use.
    #[serde(default = "default_merge_strategy")]
    pub merge_strategy: String,
}

/// Default merge strategy function.
#[must_use]
pub fn default_merge_strategy() -> String {
    "union".to_string()
}

/// Execution strategy for agent tasks within a branch.
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(tag = "type")]
pub enum ExecutionStrategyRequest {
    /// Execute agents sequentially in order.
    #[default]
    #[serde(rename = "sequential")]
    Sequential,
    /// Execute agents in parallel with optional concurrency limit.
    #[serde(rename = "parallel")]
    Parallel {
        /// Maximum number of concurrent agents that can run simultaneously.
        /// If None, uses system default based on available resources.
        max_concurrent: Option<usize>,
    },
}

/// Agent assignment within a branch.
#[derive(Debug, Clone, Deserialize)]
pub struct AgentAssignmentRequest {
    /// ID of the agent to assign.
    pub agent_id: String,
    /// Optional task description override.
    pub task_override: Option<String>,
    /// Priority level (higher = executed first in sequential mode).
    #[serde(default)]
    pub priority: u8,
}

/// Branch response payload.
#[derive(Debug, Clone, Serialize)]
pub struct BranchResponse {
    /// Branch unique identifier.
    pub id: String,
    /// Parent branch ID (None for root branches).
    pub parent_id: Option<String>,
    /// Branch name.
    pub name: String,
    /// Current status (pending, running, completed, failed, aborted).
    pub status: String,
    /// Session ID for VFS isolation.
    pub session_id: String,
    /// Creation timestamp (ISO8601).
    pub created_at: String,
    /// Completion timestamp (ISO8601), None if not completed.
    pub completed_at: Option<String>,
    /// Child branch IDs.
    pub children: Vec<String>,
}

/// Execute branch request.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct ExecuteBranchRequest {
    /// Specific agents to execute (empty = all assigned agents).
    #[serde(default)]
    pub agents: Vec<String>,
    /// Optional task description override.
    #[serde(default)]
    pub task_description: Option<String>,
}

/// Merge request payload.
#[derive(Debug, Clone, Deserialize)]
pub struct MergeRequest {
    /// Merge strategy to apply.
    #[serde(default = "default_merge_strategy")]
    pub strategy: String,
    /// Whether approval is required before merging.
    #[serde(default)]
    pub requires_approval: bool,
}

/// Merge response payload.
#[derive(Debug, Clone, Serialize)]
pub struct MergeResponse {
    /// Merge request unique identifier.
    pub merge_request_id: String,
    /// Current status (pending, approved, rejected, merged, conflict).
    pub status: String,
    /// Whether approval is still required.
    pub requires_approval: bool,
    /// Conflicts if any detected.
    pub conflicts: Option<Vec<ConflictResponse>>,
}

/// Conflict information for merge operations.
#[derive(Debug, Clone, Serialize)]
pub struct ConflictResponse {
    /// File path where conflict occurred.
    pub file_path: String,
    /// Type of conflict.
    pub conflict_type: String,
    /// Branch IDs involved in the conflict.
    pub branches: Vec<String>,
}

/// Branch tree response.
#[derive(Debug, Clone, Serialize)]
pub struct BranchTreeResponse {
    /// Root node of the tree.
    pub root: BranchNodeResponse,
}

/// Node in the branch tree hierarchy.
#[derive(Debug, Clone, Serialize)]
pub struct BranchNodeResponse {
    /// Branch ID.
    pub id: String,
    /// Branch name.
    pub name: String,
    /// Branch status.
    pub status: String,
    /// Child nodes.
    pub children: Vec<BranchNodeResponse>,
}

/// Query parameters for listing branches.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct ListBranchesQuery {
    /// Filter by status.
    pub status: Option<String>,
    /// Filter by parent branch ID.
    pub parent_id: Option<String>,
}

/// Request to reject a merge.
#[derive(Debug, Clone, Deserialize)]
pub struct RejectMergeRequest {
    /// Reason for rejection (optional).
    pub reason: Option<String>,
}

/// Convert a Branch domain model to API response.
#[must_use]
pub fn branch_to_response(branch: &Branch) -> BranchResponse {
    BranchResponse {
        id: branch.id.as_str().to_string(),
        parent_id: branch.parent_id.as_ref().map(|p| p.as_str().to_string()),
        name: branch.name.clone(),
        status: branch.status.to_string(),
        session_id: branch.session_id.clone(),
        created_at: branch.created_at.to_rfc3339(),
        completed_at: branch.completed_at.map(|t| t.to_rfc3339()),
        children: branch
            .children
            .iter()
            .map(|c| c.as_str().to_string())
            .collect(),
    }
}
