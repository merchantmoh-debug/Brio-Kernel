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
pub mod branching;
pub mod merging;
pub mod pending;

pub use analyzing::{AnalyzingForBranchHandler, merge_strategy};
pub use branching::BranchingHandler;
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
    use super::*;
    use crate::branch::MergeRequestId;
    use crate::domain::{
        AgentId, BranchConfig, BranchId, BranchStatus, MergeStatus, Priority, Task, TaskId,
        TaskStatus,
    };
    use std::collections::HashSet;

    /// Mock BranchManager for testing.
    ///
    /// Uses interior mutability to allow modifications through immutable references,
    /// which is required since BranchManager trait methods take &self.
    /// Uses RwLock instead of RefCell to satisfy Send + Sync requirements.
    use std::sync::RwLock;

    struct MockBranchManager {
        branches: RwLock<HashMap<BranchId, BranchStatus>>,
        merges: RwLock<HashMap<MergeRequestId, (MergeStatus, Vec<Conflict>)>>,
        next_branch_id: RwLock<u64>,
    }

    impl MockBranchManager {
        fn new() -> Self {
            Self {
                branches: RwLock::new(HashMap::new()),
                merges: RwLock::new(HashMap::new()),
                next_branch_id: RwLock::new(1),
            }
        }

        fn complete_branch(&self, branch_id: BranchId) {
            self.branches
                .write()
                .unwrap()
                .insert(branch_id, BranchStatus::Completed);
        }

        fn fail_branch(&self, branch_id: BranchId) {
            self.branches
                .write()
                .unwrap()
                .insert(branch_id, BranchStatus::Failed);
        }

        fn approve_merge_request(&self, merge_id: MergeRequestId) {
            if let Some((status, _)) = self.merges.write().unwrap().get_mut(&merge_id) {
                *status = MergeStatus::Approved;
            }
        }

        fn complete_merge(&self, merge_id: MergeRequestId) {
            if let Some((status, _)) = self.merges.write().unwrap().get_mut(&merge_id) {
                *status = MergeStatus::Merged;
            }
        }
    }

    impl BranchManager for MockBranchManager {
        fn create_branch(
            &self,
            _parent_branch_id: Option<BranchId>,
            _name: String,
            _config: BranchConfig,
        ) -> Result<BranchId, BranchManagerError> {
            let mut id_ref = self.next_branch_id.write().unwrap();
            let id = BranchId::from_uuid(uuid::Uuid::from_u128(*id_ref as u128));
            *id_ref += 1;
            self.branches
                .write()
                .unwrap()
                .insert(id, BranchStatus::Pending);
            Ok(id)
        }

        fn get_branch_status(
            &self,
            branch_id: BranchId,
        ) -> Result<BranchStatus, BranchManagerError> {
            self.branches
                .read()
                .unwrap()
                .get(&branch_id)
                .copied()
                .ok_or_else(|| {
                    BranchManagerError::QueryError(format!("Branch {branch_id} not found"))
                })
        }

        fn get_branch_statuses(
            &self,
            branch_ids: &[BranchId],
        ) -> Result<HashMap<BranchId, BranchStatus>, BranchManagerError> {
            let mut result = HashMap::new();
            let branches = self.branches.read().unwrap();
            for id in branch_ids {
                if let Some(status) = branches.get(id) {
                    result.insert(*id, *status);
                }
            }
            Ok(result)
        }

        fn create_merge_request(
            &self,
            _branches: &[BranchId],
            _strategy: &str,
            _requires_approval: bool,
        ) -> Result<MergeRequestId, BranchManagerError> {
            let id = MergeRequestId::new();
            self.merges
                .write()
                .unwrap()
                .insert(id, (MergeStatus::Pending, Vec::new()));
            Ok(id)
        }

        fn get_merge_status(
            &self,
            merge_request_id: MergeRequestId,
        ) -> Result<MergeStatus, BranchManagerError> {
            self.merges
                .read()
                .unwrap()
                .get(&merge_request_id)
                .map(|(status, _)| *status)
                .ok_or_else(|| {
                    BranchManagerError::QueryError(format!("Merge {merge_request_id} not found"))
                })
        }

        fn get_merge_conflicts(
            &self,
            merge_request_id: MergeRequestId,
        ) -> Result<Vec<Conflict>, BranchManagerError> {
            self.merges
                .read()
                .unwrap()
                .get(&merge_request_id)
                .map(|(_, conflicts)| conflicts.clone())
                .ok_or_else(|| {
                    BranchManagerError::QueryError(format!("Merge {merge_request_id} not found"))
                })
        }

        fn approve_merge(
            &self,
            merge_request_id: MergeRequestId,
            _approver: &str,
        ) -> Result<(), BranchManagerError> {
            if let Some((status, _)) = self.merges.write().unwrap().get_mut(&merge_request_id) {
                *status = MergeStatus::Approved;
                Ok(())
            } else {
                Err(BranchManagerError::ApprovalError(format!(
                    "Merge {merge_request_id} not found"
                )))
            }
        }

        fn execute_merge(
            &self,
            merge_request_id: MergeRequestId,
        ) -> Result<(), BranchManagerError> {
            if let Some((status, _)) = self.merges.write().unwrap().get_mut(&merge_request_id) {
                *status = MergeStatus::Merged;
                Ok(())
            } else {
                Err(BranchManagerError::MergeRequestError(format!(
                    "Merge {merge_request_id} not found"
                )))
            }
        }

        fn is_merge_pending_approval(&self, merge_request_id: MergeRequestId) -> bool {
            self.merges
                .read()
                .unwrap()
                .get(&merge_request_id)
                .map(|(status, _)| *status == MergeStatus::Pending)
                .unwrap_or(false)
        }

        fn execute_branches(&self, branches: &[BranchId]) -> Result<(), BranchManagerError> {
            let mut branches_mut = self.branches.write().unwrap();
            for id in branches {
                branches_mut.insert(*id, BranchStatus::Active);
            }
            Ok(())
        }
    }

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
