//! Handler for branching task states.
//!
//! These handlers manage the branching lifecycle:
//! - `AnalyzingForBranch`: Determine if task needs branching
//! - Branching: Monitor branch execution progress
//! - Merging: Handle merge operations
//! - `MergePendingApproval`: Wait for manual approval

use crate::branch::MergeRequestId;
use crate::domain::{BranchConfig, BranchId, BranchStatus, Conflict, MergeStatus};
use crate::orchestrator::SupervisorError;
use crate::repository::RepositoryError;
use std::collections::HashMap;

pub mod analyzing;
pub mod execution;
pub mod merging;
pub mod pending;

pub use analyzing::{AnalyzingForBranchHandler, merge_strategy};
pub use execution::BranchingHandler;
pub use merging::MergingHandler;
pub use pending::MergePendingApprovalHandler;

/// Extension trait for converting `BranchManagerError` to `SupervisorError`.
pub trait BranchManagerErrorExt {
    /// Converts the error to a `SupervisorError::RepositoryFailure`.
    fn to_supervisor_error(self) -> SupervisorError;
}

impl BranchManagerErrorExt for BranchManagerError {
    fn to_supervisor_error(self) -> SupervisorError {
        SupervisorError::RepositoryFailure(RepositoryError::SqlError(self.to_string()))
    }
}

/// Error type for branch manager operations.
#[derive(Debug)]
pub enum BranchManagerError {
    /// Branch creation failed.
    CreateError(String),
    /// Branch query failed.
    QueryError(String),
    /// Merge request creation failed.
    MergeRequestError(String),
    /// Merge approval failed.
    ApprovalError(String),
}

impl core::fmt::Display for BranchManagerError {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        match self {
            Self::CreateError(msg) => write!(f, "Branch creation error: {msg}"),
            Self::QueryError(msg) => write!(f, "Branch query error: {msg}"),
            Self::MergeRequestError(msg) => write!(f, "Merge request error: {msg}"),
            Self::ApprovalError(msg) => write!(f, "Approval error: {msg}"),
        }
    }
}

impl std::error::Error for BranchManagerError {}

/// Trait for managing branches.
///
/// This trait abstracts branch lifecycle operations, enabling:
/// - Creating and managing branches
/// - Querying branch statuses
/// - Creating and managing merge requests
pub trait BranchManager: Send + Sync {
    /// Creates a new branch for task execution.
    ///
    /// # Errors
    /// Returns `BranchManagerError` if branch creation fails.
    fn create_branch(
        &self,
        parent_branch_id: Option<BranchId>,
        name: String,
        config: BranchConfig,
    ) -> Result<BranchId, BranchManagerError>;

    /// Gets the current status of a branch.
    ///
    /// # Errors
    /// Returns `BranchManagerError` if the query fails.
    fn get_branch_status(&self, branch_id: BranchId) -> Result<BranchStatus, BranchManagerError>;

    /// Gets the status of multiple branches.
    ///
    /// # Errors
    /// Returns `BranchManagerError` if the query fails.
    fn get_branch_statuses(
        &self,
        branch_ids: &[BranchId],
    ) -> Result<HashMap<BranchId, BranchStatus>, BranchManagerError>;

    /// Creates a merge request for completed branches.
    ///
    /// # Errors
    /// Returns `BranchManagerError` if merge request creation fails.
    fn create_merge_request(
        &self,
        branches: &[BranchId],
        strategy: &str,
        requires_approval: bool,
    ) -> Result<MergeRequestId, BranchManagerError>;

    /// Gets the current status of a merge request.
    ///
    /// # Errors
    /// Returns `BranchManagerError` if the query fails.
    fn get_merge_status(
        &self,
        merge_request_id: MergeRequestId,
    ) -> Result<MergeStatus, BranchManagerError>;

    /// Gets conflicts for a merge request.
    ///
    /// # Errors
    /// Returns `BranchManagerError` if the query fails.
    fn get_merge_conflicts(
        &self,
        merge_request_id: MergeRequestId,
    ) -> Result<Vec<Conflict>, BranchManagerError>;

    /// Approves a merge request.
    ///
    /// # Errors
    /// Returns `BranchManagerError` if the approval fails.
    fn approve_merge(
        &self,
        merge_request_id: MergeRequestId,
        approver: &str,
    ) -> Result<(), BranchManagerError>;

    /// Executes a merge after approval.
    ///
    /// # Errors
    /// Returns `BranchManagerError` if the merge execution fails.
    fn execute_merge(&self, merge_request_id: MergeRequestId) -> Result<(), BranchManagerError>;

    /// Checks if a merge request is pending approval.
    fn is_merge_pending_approval(&self, merge_request_id: MergeRequestId) -> bool;

    /// Executes branches by dispatching tasks to agents.
    ///
    /// # Errors
    /// Returns `BranchManagerError` if execution fails.
    fn execute_branches(&self, branches: &[BranchId]) -> Result<(), BranchManagerError>;
}

/// No-op `BranchManager` for testing or when branching is disabled.
pub struct NoOpBranchManager;

impl BranchManager for NoOpBranchManager {
    fn create_branch(
        &self,
        _parent_branch_id: Option<BranchId>,
        _name: String,
        _config: BranchConfig,
    ) -> Result<BranchId, BranchManagerError> {
        Err(BranchManagerError::CreateError(
            "NoOpBranchManager does not support branch creation".to_string(),
        ))
    }

    fn get_branch_status(&self, _branch_id: BranchId) -> Result<BranchStatus, BranchManagerError> {
        Err(BranchManagerError::QueryError(
            "NoOpBranchManager does not support branch queries".to_string(),
        ))
    }

    fn get_branch_statuses(
        &self,
        _branch_ids: &[BranchId],
    ) -> Result<HashMap<BranchId, BranchStatus>, BranchManagerError> {
        Err(BranchManagerError::QueryError(
            "NoOpBranchManager does not support branch queries".to_string(),
        ))
    }

    fn create_merge_request(
        &self,
        _branches: &[BranchId],
        _strategy: &str,
        _requires_approval: bool,
    ) -> Result<MergeRequestId, BranchManagerError> {
        Err(BranchManagerError::MergeRequestError(
            "NoOpBranchManager does not support merge requests".to_string(),
        ))
    }

    fn get_merge_status(
        &self,
        _merge_request_id: MergeRequestId,
    ) -> Result<MergeStatus, BranchManagerError> {
        Err(BranchManagerError::QueryError(
            "NoOpBranchManager does not support merge queries".to_string(),
        ))
    }

    fn get_merge_conflicts(
        &self,
        _merge_request_id: MergeRequestId,
    ) -> Result<Vec<Conflict>, BranchManagerError> {
        Ok(Vec::new())
    }

    fn approve_merge(
        &self,
        _merge_request_id: MergeRequestId,
        _approver: &str,
    ) -> Result<(), BranchManagerError> {
        Err(BranchManagerError::ApprovalError(
            "NoOpBranchManager does not support merge approval".to_string(),
        ))
    }

    fn execute_merge(&self, _merge_request_id: MergeRequestId) -> Result<(), BranchManagerError> {
        Err(BranchManagerError::MergeRequestError(
            "NoOpBranchManager does not support merge execution".to_string(),
        ))
    }

    fn is_merge_pending_approval(&self, _merge_request_id: MergeRequestId) -> bool {
        false
    }

    fn execute_branches(&self, _branches: &[BranchId]) -> Result<(), BranchManagerError> {
        Err(BranchManagerError::CreateError(
            "NoOpBranchManager does not support branch execution".to_string(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use crate::domain::{Priority, Task, TaskId, TaskStatus};
    use std::collections::HashSet;

    fn test_task_with_content(id: u64, content: &str, status: TaskStatus) -> Task {
        Task::new(
            TaskId::new(id),
            content.to_string(),
            Priority::DEFAULT,
            status,
            None,
            None,
            HashSet::new(),
        )
        .expect("test task should be valid")
    }

    #[test]
    fn test_should_use_branching_detection() {
        let task1 = test_task_with_content(
            1,
            "Multiple reviewers needed for security and performance review",
            TaskStatus::Pending,
        );
        assert_eq!(
            crate::domain::should_use_branching(&task1),
            Some(crate::domain::BranchingStrategy::MultipleReviewers)
        );

        let task2 = test_task_with_content(
            2,
            "Implement both approaches for A/B testing",
            TaskStatus::Pending,
        );
        assert_eq!(
            crate::domain::should_use_branching(&task2),
            Some(crate::domain::BranchingStrategy::AlternativeImplementations)
        );

        let task3 = test_task_with_content(
            3,
            "Refactor codebase with sub-tasks for each module",
            TaskStatus::Pending,
        );
        assert_eq!(
            crate::domain::should_use_branching(&task3),
            Some(crate::domain::BranchingStrategy::NestedBranches)
        );

        let task4 = test_task_with_content(4, "Simple bug fix", TaskStatus::Pending);
        assert_eq!(crate::domain::should_use_branching(&task4), None);
    }
}
