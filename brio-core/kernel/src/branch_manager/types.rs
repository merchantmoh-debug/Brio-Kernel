//! Branch types for the Brio kernel.
//!
//! This module provides domain types for managing branches in the Brio system.

use chrono::{DateTime, Utc};

/// Branch identifier.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BranchId(String);

impl BranchId {
    /// Create a new branch ID from a string.
    ///
    /// # Errors
    ///
    /// Returns an error if the ID is empty.
    pub fn new(id: String) -> Result<Self, BranchError> {
        if id.is_empty() {
            return Err(BranchError::Internal("empty ID".to_string()));
        }
        // Validate UUID-like format (simple check)
        if id.len() != 36 && !id.chars().all(|c| c.is_alphanumeric() || c == '-') {
            return Err(BranchError::Internal(format!("invalid ID format: {id}")));
        }
        Ok(Self(id))
    }

    /// Get the string representation.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for BranchId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl TryFrom<String> for BranchId {
    type Error = BranchError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

/// Merge request identifier.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MergeRequestId(String);

impl MergeRequestId {
    /// Create a new merge request ID from a string.
    ///
    /// # Errors
    ///
    /// Returns an error if the ID is empty.
    pub fn new(id: String) -> Result<Self, BranchError> {
        if id.is_empty() {
            return Err(BranchError::Internal("empty ID".to_string()));
        }
        Ok(Self(id))
    }

    /// Get the string representation.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for MergeRequestId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Branch-related errors.
#[derive(Debug, thiserror::Error)]
pub enum BranchError {
    /// Branch not found.
    #[error("Branch not found: {0}")]
    BranchNotFound(String),
    /// Maximum number of branches exceeded.
    #[error("Maximum branches exceeded: {current}/{limit}")]
    MaxBranchesExceeded {
        /// Current number of branches.
        current: usize,
        /// Maximum allowed branches.
        limit: usize,
    },
    /// Branch with this name already exists.
    #[error("Branch already exists: {0}")]
    BranchAlreadyExists(String),
    /// Invalid state transition.
    #[error("Invalid state transition from {from} to {to}")]
    InvalidStateTransition {
        /// Current state name.
        from: String,
        /// Target state name.
        to: String,
    },
    /// Execution failed.
    #[error("Execution failed: {0}")]
    ExecutionFailed(String),
    /// Merge conflict detected.
    #[error("Merge conflict in {file_path}: {description}")]
    MergeConflict {
        /// Path to the file with conflict.
        file_path: String,
        /// Description of the conflict.
        description: String,
    },
    /// Database error.
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
    /// Internal error.
    #[error("Internal error: {0}")]
    Internal(String),
}

/// Branch status.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum BranchStatus {
    /// Branch created but not yet executed.
    #[default]
    Pending,
    /// Branch is currently executing.
    Running,
    /// Branch completed successfully.
    Completed,
    /// Branch execution failed.
    Failed,
    /// Branch was aborted.
    Aborted,
    /// Branch is being merged.
    Merging,
}

impl std::fmt::Display for BranchStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BranchStatus::Pending => write!(f, "pending"),
            BranchStatus::Running => write!(f, "running"),
            BranchStatus::Completed => write!(f, "completed"),
            BranchStatus::Failed => write!(f, "failed"),
            BranchStatus::Aborted => write!(f, "aborted"),
            BranchStatus::Merging => write!(f, "merging"),
        }
    }
}

/// Branch domain model.
#[derive(Debug, Clone)]
pub struct Branch {
    /// Branch ID.
    pub id: BranchId,
    /// Parent branch ID.
    pub parent_id: Option<BranchId>,
    /// Branch name.
    pub name: String,
    /// Current status.
    pub status: BranchStatus,
    /// VFS session ID.
    pub session_id: String,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
    /// Completion timestamp.
    pub completed_at: Option<DateTime<Utc>>,
    /// Child branch IDs.
    pub children: Vec<BranchId>,
    /// Branch configuration.
    pub config: BranchConfig,
}

/// Branch configuration.
#[derive(Debug, Clone)]
pub struct BranchConfig {
    /// Agent assignments.
    pub agents: Vec<AgentAssignment>,
    /// Execution strategy.
    pub execution_strategy: ExecutionStrategy,
    /// Auto-merge flag.
    pub auto_merge: bool,
    /// Merge strategy.
    pub merge_strategy: String,
}

/// Execution strategy.
#[derive(Debug, Clone)]
pub enum ExecutionStrategy {
    /// Sequential execution.
    Sequential,
    /// Parallel execution with concurrency limit.
    Parallel {
        /// Maximum number of concurrent agents. If None, uses system default.
        max_concurrent: Option<usize>,
    },
}

/// Agent assignment.
#[derive(Debug, Clone)]
pub struct AgentAssignment {
    /// Agent ID.
    pub agent_id: String,
    /// Task override.
    pub task_override: Option<String>,
    /// Priority level.
    pub priority: u8,
}

/// Merge request domain model.
#[derive(Debug, Clone)]
pub struct MergeRequestModel {
    /// Merge request ID.
    pub id: MergeRequestId,
    /// Source branch ID.
    pub branch_id: BranchId,
    /// Merge strategy.
    pub strategy: String,
    /// Current status.
    pub status: MergeRequestStatus,
    /// Whether approval is required.
    pub requires_approval: bool,
    /// Approval information.
    pub approved_by: Option<String>,
    /// Approval timestamp.
    pub approved_at: Option<DateTime<Utc>>,
}

/// Merge request status.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MergeRequestStatus {
    /// Pending approval or execution.
    Pending,
    /// Approved but not yet merged.
    Approved,
    /// Rejected.
    Rejected,
    /// Successfully merged.
    Merged,
    /// Conflict detected.
    Conflict,
}

impl std::fmt::Display for MergeRequestStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MergeRequestStatus::Pending => write!(f, "pending"),
            MergeRequestStatus::Approved => write!(f, "approved"),
            MergeRequestStatus::Rejected => write!(f, "rejected"),
            MergeRequestStatus::Merged => write!(f, "merged"),
            MergeRequestStatus::Conflict => write!(f, "conflict"),
        }
    }
}
