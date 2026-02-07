//! Branch manager for the Brio kernel.
//!
//! This module provides the BranchManager and related domain types for
//! managing branches in the Brio system. It is separate from the API layer
//! to avoid circular dependencies.

use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use std::collections::HashMap;

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
        max_concurrent: Option<usize>
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

// =============================================================================
// Branch Manager
// =============================================================================

/// Manager for branch operations.
#[derive(Debug, Default)]
pub struct BranchManager {
    /// In-memory storage (temporary - will use database).
    branches: RwLock<HashMap<String, Branch>>,
    merge_requests: RwLock<HashMap<String, MergeRequestModel>>,
}

impl BranchManager {
    /// Create a new branch manager.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a new branch.
    ///
    /// # Errors
    ///
    /// Returns an error if the branch cannot be created.
    pub async fn create_branch(
        &self,
        name: String,
        agents: Vec<AgentAssignment>,
        execution_strategy: ExecutionStrategy,
        auto_merge: bool,
        merge_strategy: String,
    ) -> Result<Branch, BranchError> {
        let id = BranchId::new(uuid::Uuid::new_v4().to_string())
            .map_err(|e| BranchError::Internal(e.to_string()))?;

        // Check for duplicate name (simplified)
        let branches = self.branches.read();
        for branch in branches.values() {
            if branch.name == name {
                return Err(BranchError::BranchAlreadyExists(name.clone()));
            }
        }
        drop(branches);

        let branch = Branch {
            id,
            parent_id: None, // TODO: Set based on source
            name,
            status: BranchStatus::Pending,
            session_id: uuid::Uuid::new_v4().to_string(),
            created_at: Utc::now(),
            completed_at: None,
            children: Vec::new(),
            config: BranchConfig {
                agents,
                execution_strategy,
                auto_merge,
                merge_strategy,
            },
        };

        self.branches
            .write()
            .insert(branch.id.as_str().to_string(), branch.clone());

        Ok(branch)
    }

    /// Get a branch by ID.
    ///
    /// # Errors
    ///
    /// Returns an error if the branch is not found.
    pub async fn get_branch(&self, id: &BranchId) -> Result<Branch, BranchError> {
        self.branches
            .read()
            .get(id.as_str())
            .cloned()
            .ok_or_else(|| BranchError::BranchNotFound(id.to_string()))
    }

    /// List branches with optional filters.
    ///
    /// # Errors
    ///
    /// Returns an error if the database query fails.
    pub async fn list_branches(
        &self,
        status_filter: Option<&str>,
        parent_id_filter: Option<&BranchId>,
    ) -> Result<Vec<Branch>, BranchError> {
        let branches = self.branches.read();
        let mut result: Vec<Branch> = branches
            .values()
            .filter(|b| {
                if let Some(status) = status_filter {
                    b.status.to_string() == status
                } else {
                    true
                }
            })
            .filter(|b| {
                if let Some(parent_id) = parent_id_filter {
                    b.parent_id
                        .as_ref()
                        .map(|p| p.as_str() == parent_id.as_str())
                        .unwrap_or(false)
                } else {
                    true
                }
            })
            .cloned()
            .collect();

        // Sort by created_at descending
        result.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        Ok(result)
    }

    /// Delete a branch.
    ///
    /// # Errors
    ///
    /// Returns an error if the branch is not found or cannot be deleted.
    pub async fn delete_branch(&self, id: &BranchId) -> Result<(), BranchError> {
        let mut branches = self.branches.write();
        if !branches.contains_key(id.as_str()) {
            return Err(BranchError::BranchNotFound(id.to_string()));
        }
        branches.remove(id.as_str());
        Ok(())
    }

    /// Execute a branch.
    ///
    /// # Errors
    ///
    /// Returns an error if the branch is not found or execution fails.
    pub async fn execute_branch(
        &self,
        id: &BranchId,
        _agent_filter: Option<Vec<String>>,
        _task_override: Option<String>,
    ) -> Result<Branch, BranchError> {
        let mut branches = self.branches.write();
        let branch = branches
            .get_mut(id.as_str())
            .ok_or_else(|| BranchError::BranchNotFound(id.to_string()))?;

        if branch.status != BranchStatus::Pending && branch.status != BranchStatus::Failed {
            return Err(BranchError::InvalidStateTransition {
                from: branch.status.to_string(),
                to: "running".to_string(),
            });
        }

        branch.status = BranchStatus::Running;
        // TODO: Actually execute the branch
        branch.status = BranchStatus::Completed;
        branch.completed_at = Some(Utc::now());

        Ok(branch.clone())
    }

    /// Abort a branch.
    ///
    /// # Errors
    ///
    /// Returns an error if the branch is not found or cannot be aborted.
    pub async fn abort_branch(&self, id: &BranchId) -> Result<Branch, BranchError> {
        let mut branches = self.branches.write();
        let branch = branches
            .get_mut(id.as_str())
            .ok_or_else(|| BranchError::BranchNotFound(id.to_string()))?;

        if branch.status != BranchStatus::Running && branch.status != BranchStatus::Pending {
            return Err(BranchError::InvalidStateTransition {
                from: branch.status.to_string(),
                to: "aborted".to_string(),
            });
        }

        branch.status = BranchStatus::Aborted;
        branch.completed_at = Some(Utc::now());

        Ok(branch.clone())
    }

    /// Request a merge for a branch.
    ///
    /// # Errors
    ///
    /// Returns an error if the branch is not found or cannot be merged.
    pub async fn request_merge(
        &self,
        branch_id: &BranchId,
        strategy: String,
        requires_approval: bool,
    ) -> Result<MergeRequestModel, BranchError> {
        let branches = self.branches.read();
        let _branch = branches
            .get(branch_id.as_str())
            .ok_or_else(|| BranchError::BranchNotFound(branch_id.to_string()))?;
        drop(branches);

        let merge_request_id = MergeRequestId::new(uuid::Uuid::new_v4().to_string())
            .map_err(|e| BranchError::Internal(e.to_string()))?;

        let merge_request = MergeRequestModel {
            id: merge_request_id,
            branch_id: branch_id.clone(),
            strategy,
            status: MergeRequestStatus::Pending,
            requires_approval,
            approved_by: None,
            approved_at: None,
        };

        self.merge_requests
            .write()
            .insert(merge_request.id.as_str().to_string(), merge_request.clone());

        Ok(merge_request)
    }

    /// Approve a merge request.
    ///
    /// # Errors
    ///
    /// Returns an error if the merge request is not found.
    pub async fn approve_merge(
        &self,
        merge_request_id: &MergeRequestId,
        approver: String,
    ) -> Result<MergeRequestModel, BranchError> {
        let mut merge_requests = self.merge_requests.write();
        let merge_request = merge_requests
            .get_mut(merge_request_id.as_str())
            .ok_or_else(|| BranchError::BranchNotFound(merge_request_id.to_string()))?;

        merge_request.status = MergeRequestStatus::Approved;
        merge_request.approved_by = Some(approver);
        merge_request.approved_at = Some(Utc::now());

        Ok(merge_request.clone())
    }

    /// Reject a merge request.
    ///
    /// # Errors
    ///
    /// Returns an error if the merge request is not found.
    pub async fn reject_merge(
        &self,
        merge_request_id: &MergeRequestId,
    ) -> Result<MergeRequestModel, BranchError> {
        let mut merge_requests = self.merge_requests.write();
        let merge_request = merge_requests
            .get_mut(merge_request_id.as_str())
            .ok_or_else(|| BranchError::BranchNotFound(merge_request_id.to_string()))?;

        merge_request.status = MergeRequestStatus::Rejected;

        Ok(merge_request.clone())
    }

    /// Get the merge request for a branch.
    ///
    /// # Errors
    ///
    /// Returns an error if the merge request is not found.
    pub async fn get_merge_request(
        &self,
        merge_request_id: &MergeRequestId,
    ) -> Result<MergeRequestModel, BranchError> {
        self.merge_requests
            .read()
            .get(merge_request_id.as_str())
            .cloned()
            .ok_or_else(|| BranchError::BranchNotFound(merge_request_id.to_string()))
    }

    /// Get all merge requests for a branch.
    ///
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub async fn get_branch_merge_requests(
        &self,
        branch_id: &BranchId,
    ) -> Result<Vec<MergeRequestModel>, BranchError> {
        let merge_requests = self.merge_requests.read();
        let result: Vec<MergeRequestModel> = merge_requests
            .values()
            .filter(|mr| mr.branch_id.as_str() == branch_id.as_str())
            .cloned()
            .collect();
        Ok(result)
    }

    /// Get the branch tree.
    ///
    /// # Errors
    ///
    /// Returns an error if the branch is not found.
    pub async fn get_branch_tree(&self, id: &BranchId) -> Result<Branch, BranchError> {
        self.get_branch(id).await
    }
}
