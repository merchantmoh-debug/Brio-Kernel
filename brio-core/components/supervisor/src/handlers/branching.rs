//! Handler for branching task states.
//!
//! These handlers manage the branching lifecycle:
//! - AnalyzingForBranch: Determine if task needs branching
//! - Branching: Monitor branch execution progress
//! - Merging: Handle merge operations
//! - MergePendingApproval: Wait for manual approval

use crate::domain::{
    BranchConfig, BranchId, BranchStatus, BranchingStrategy, Conflict, MergeRequestId, MergeStatus,
    Task, TaskStatus,
};
use crate::handlers::{SupervisorContext, TaskStateHandler};
use crate::mesh_client::AgentDispatcher;
use crate::orchestrator::{Planner, SupervisorError};
use crate::repository::{RepositoryError, TaskRepository};
use crate::selector::AgentSelector;
use std::collections::HashMap;

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
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
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

/// Handler for AnalyzingForBranch state.
///
/// Analyzes task content to determine if branching is needed and creates
/// branches if applicable.
pub struct AnalyzingForBranchHandler;

impl<R, D, P, S> TaskStateHandler<R, D, P, S> for AnalyzingForBranchHandler
where
    R: TaskRepository,
    D: AgentDispatcher,
    P: Planner,
    S: AgentSelector,
{
    fn handle(
        &self,
        ctx: &SupervisorContext<R, D, P, S>,
        task: &Task,
    ) -> Result<bool, SupervisorError> {
        // Check if branch manager is available
        let Some(branch_manager) = ctx.branch_manager.as_ref() else {
            // No branch manager configured, proceed to Executing
            ctx.repository
                .update_status(task.id(), TaskStatus::Executing)
                .map_err(SupervisorError::StatusUpdateFailure)?;
            return Ok(true);
        };

        // Analyze task to determine branching strategy
        match crate::domain::should_use_branching(task) {
            Some(strategy) => {
                // Create branches based on strategy
                let branches = create_branches_for_strategy(
                    branch_manager.as_ref(),
                    task,
                    strategy,
                )
                .map_err(|e| {
                    SupervisorError::RepositoryFailure(RepositoryError::SqlError(e.to_string()))
                })?;

                if branches.is_empty() {
                    // No branches created, fall back to Executing
                    ctx.repository
                        .update_status(task.id(), TaskStatus::Executing)
                        .map_err(SupervisorError::StatusUpdateFailure)?;
                    Ok(true)
                } else {
                    // Transition to Branching state
                    ctx.repository
                        .update_status(
                            task.id(),
                            TaskStatus::Branching {
                                branches: branches.clone(),
                                completed: 0,
                                total: branches.len(),
                            },
                        )
                        .map_err(SupervisorError::StatusUpdateFailure)?;

                    // Start executing branches
                    branch_manager.execute_branches(&branches).map_err(|e| {
                        SupervisorError::RepositoryFailure(RepositoryError::SqlError(e.to_string()))
                    })?;

                    Ok(true)
                }
            }
            None => {
                // No branching needed, transition to Executing
                ctx.repository
                    .update_status(task.id(), TaskStatus::Executing)
                    .map_err(SupervisorError::StatusUpdateFailure)?;
                Ok(true)
            }
        }
    }
}

/// Creates branches based on the branching strategy.
fn create_branches_for_strategy(
    branch_manager: &dyn BranchManager,
    task: &Task,
    strategy: BranchingStrategy,
) -> Result<Vec<BranchId>, BranchManagerError> {
    let mut branches = Vec::new();

    match strategy {
        BranchingStrategy::MultipleReviewers => {
            // Create branches for different reviewer perspectives
            let reviewers = vec!["security-reviewer", "performance-reviewer", "code-reviewer"];
            for reviewer in reviewers {
                let config = BranchConfig::new(
                    format!("{}-{}", task.id(), reviewer),
                    vec![],
                    crate::domain::ExecutionStrategy::Sequential,
                    false,
                    "three-way",
                )
                .map_err(|e| {
                    BranchManagerError::CreateError(format!("Failed to create branch config: {e}"))
                })?;

                let branch_id =
                    branch_manager.create_branch(None, config.name().to_string(), config)?;
                branches.push(branch_id);
            }
        }
        BranchingStrategy::AlternativeImplementations => {
            // Create branches for alternative approaches
            let approaches = vec!["approach-a", "approach-b"];
            for approach in approaches {
                let config = BranchConfig::new(
                    format!("{}-{}", task.id(), approach),
                    vec![],
                    crate::domain::ExecutionStrategy::Sequential,
                    false,
                    "three-way",
                )
                .map_err(|e| {
                    BranchManagerError::CreateError(format!("Failed to create branch config: {e}"))
                })?;

                let branch_id =
                    branch_manager.create_branch(None, config.name().to_string(), config)?;
                branches.push(branch_id);
            }
        }
        BranchingStrategy::NestedBranches => {
            // Create branches for sub-tasks
            // For now, create a single branch - in practice, this would parse
            // the task content to identify sub-tasks
            let config = BranchConfig::new(
                format!("{}-refactor", task.id()),
                vec![],
                crate::domain::ExecutionStrategy::Sequential,
                false,
                "three-way",
            )
            .map_err(|e| {
                BranchManagerError::CreateError(format!("Failed to create branch config: {e}"))
            })?;

            let branch_id =
                branch_manager.create_branch(None, config.name().to_string(), config)?;
            branches.push(branch_id);
        }
    }

    Ok(branches)
}

/// Handler for Branching state.
///
/// Monitors branch execution progress and transitions to Merging or
/// MergePendingApproval when branches complete.
pub struct BranchingHandler;

impl<R, D, P, S> TaskStateHandler<R, D, P, S> for BranchingHandler
where
    R: TaskRepository,
    D: AgentDispatcher,
    P: Planner,
    S: AgentSelector,
{
    fn handle(
        &self,
        ctx: &SupervisorContext<R, D, P, S>,
        task: &Task,
    ) -> Result<bool, SupervisorError> {
        let TaskStatus::Branching {
            branches,
            completed,
            total,
        } = task.status().clone()
        else {
            return Ok(false);
        };

        let Some(branch_manager) = ctx.branch_manager.as_ref() else {
            // No branch manager, mark task as failed
            ctx.repository
                .mark_failed(task.id(), "Branch manager not available")
                .map_err(SupervisorError::StatusUpdateFailure)?;
            return Ok(true);
        };

        // Query branch statuses
        let statuses = branch_manager.get_branch_statuses(&branches).map_err(|e| {
            SupervisorError::RepositoryFailure(RepositoryError::SqlError(e.to_string()))
        })?;

        // Count completed and failed branches
        let completed_count = statuses
            .values()
            .filter(|s| **s == BranchStatus::Completed)
            .count();
        let failed_count = statuses
            .values()
            .filter(|s| **s == BranchStatus::Failed)
            .count();

        // If any branch failed, mark task as failed
        if failed_count > 0 {
            ctx.repository
                .mark_failed(task.id(), &format!("{failed_count} branch(es) failed"))
                .map_err(SupervisorError::StatusUpdateFailure)?;
            return Ok(true);
        }

        // Check if all branches completed
        if completed_count == branches.len() {
            // All branches completed - check if auto-merge is enabled
            // For simplicity, we use auto-merge based on a heuristic
            // In production, this would be configurable per task
            let requires_approval = branches.len() > 2; // Large merges need approval

            let merge_request_id = branch_manager
                .create_merge_request(&branches, "union", requires_approval)
                .map_err(|e| {
                    SupervisorError::RepositoryFailure(RepositoryError::SqlError(e.to_string()))
                })?;

            if requires_approval {
                // Get conflicts for display
                let conflicts = branch_manager
                    .get_merge_conflicts(merge_request_id)
                    .map_err(|e| {
                        SupervisorError::RepositoryFailure(RepositoryError::SqlError(e.to_string()))
                    })?;

                ctx.repository
                    .update_status(
                        task.id(),
                        TaskStatus::MergePendingApproval {
                            branches: branches.clone(),
                            merge_request_id,
                            conflicts,
                        },
                    )
                    .map_err(SupervisorError::StatusUpdateFailure)?;
            } else {
                ctx.repository
                    .update_status(
                        task.id(),
                        TaskStatus::Merging {
                            branches: branches.clone(),
                            merge_request_id,
                        },
                    )
                    .map_err(SupervisorError::StatusUpdateFailure)?;
            }
            Ok(true)
        } else if completed_count > completed {
            // Progress updated
            ctx.repository
                .update_status(
                    task.id(),
                    TaskStatus::Branching {
                        branches: branches.clone(),
                        completed: completed_count,
                        total,
                    },
                )
                .map_err(SupervisorError::StatusUpdateFailure)?;
            Ok(true)
        } else {
            // Still executing
            Ok(false)
        }
    }
}

/// Handler for Merging state.
///
/// Monitors merge operations and completes tasks or handles conflicts.
pub struct MergingHandler;

impl<R, D, P, S> TaskStateHandler<R, D, P, S> for MergingHandler
where
    R: TaskRepository,
    D: AgentDispatcher,
    P: Planner,
    S: AgentSelector,
{
    fn handle(
        &self,
        ctx: &SupervisorContext<R, D, P, S>,
        task: &Task,
    ) -> Result<bool, SupervisorError> {
        let TaskStatus::Merging {
            branches,
            merge_request_id,
        } = task.status().clone()
        else {
            return Ok(false);
        };

        let Some(branch_manager) = ctx.branch_manager.as_ref() else {
            ctx.repository
                .mark_failed(task.id(), "Branch manager not available")
                .map_err(SupervisorError::StatusUpdateFailure)?;
            return Ok(true);
        };

        // Check merge status
        let merge_status = branch_manager
            .get_merge_status(merge_request_id)
            .map_err(|e| {
                SupervisorError::RepositoryFailure(RepositoryError::SqlError(e.to_string()))
            })?;

        match merge_status {
            MergeStatus::Merged => {
                // Merge completed successfully
                ctx.repository
                    .mark_completed(task.id())
                    .map_err(SupervisorError::StatusUpdateFailure)?;
                Ok(true)
            }
            MergeStatus::Conflict => {
                // Merge has conflicts - transition to pending approval
                let conflicts = branch_manager
                    .get_merge_conflicts(merge_request_id)
                    .map_err(|e| {
                        SupervisorError::RepositoryFailure(RepositoryError::SqlError(e.to_string()))
                    })?;

                ctx.repository
                    .update_status(
                        task.id(),
                        TaskStatus::MergePendingApproval {
                            branches: branches.clone(),
                            merge_request_id,
                            conflicts,
                        },
                    )
                    .map_err(SupervisorError::StatusUpdateFailure)?;
                Ok(true)
            }
            MergeStatus::Pending | MergeStatus::Approved => {
                // Merge still in progress
                Ok(false)
            }
            MergeStatus::Rejected => {
                // Merge was rejected
                ctx.repository
                    .mark_failed(task.id(), "Merge request was rejected")
                    .map_err(SupervisorError::StatusUpdateFailure)?;
                Ok(true)
            }
        }
    }
}

/// Handler for MergePendingApproval state.
///
/// Waits for external approval via REST API or WebSocket command.
pub struct MergePendingApprovalHandler;

impl<R, D, P, S> TaskStateHandler<R, D, P, S> for MergePendingApprovalHandler
where
    R: TaskRepository,
    D: AgentDispatcher,
    P: Planner,
    S: AgentSelector,
{
    fn handle(
        &self,
        ctx: &SupervisorContext<R, D, P, S>,
        task: &Task,
    ) -> Result<bool, SupervisorError> {
        let TaskStatus::MergePendingApproval {
            branches,
            merge_request_id,
            conflicts: _,
        } = task.status().clone()
        else {
            return Ok(false);
        };

        let Some(branch_manager) = ctx.branch_manager.as_ref() else {
            ctx.repository
                .mark_failed(task.id(), "Branch manager not available")
                .map_err(SupervisorError::StatusUpdateFailure)?;
            return Ok(true);
        };

        // Check current merge status
        let merge_status = branch_manager
            .get_merge_status(merge_request_id)
            .map_err(|e| {
                SupervisorError::RepositoryFailure(RepositoryError::SqlError(e.to_string()))
            })?;

        match merge_status {
            MergeStatus::Approved => {
                // Merge was approved - execute it
                branch_manager
                    .execute_merge(merge_request_id)
                    .map_err(|e| {
                        SupervisorError::RepositoryFailure(RepositoryError::SqlError(e.to_string()))
                    })?;

                // Transition to Merging state
                ctx.repository
                    .update_status(
                        task.id(),
                        TaskStatus::Merging {
                            branches: branches.clone(),
                            merge_request_id,
                        },
                    )
                    .map_err(SupervisorError::StatusUpdateFailure)?;
                Ok(true)
            }
            MergeStatus::Rejected => {
                // Merge was rejected
                ctx.repository
                    .mark_failed(task.id(), "Merge request was rejected")
                    .map_err(SupervisorError::StatusUpdateFailure)?;
                Ok(true)
            }
            MergeStatus::Merged => {
                // Already merged (approved and executed externally)
                ctx.repository
                    .mark_completed(task.id())
                    .map_err(SupervisorError::StatusUpdateFailure)?;
                Ok(true)
            }
            _ => {
                // Still pending approval - wait
                // External approval comes via REST API or WebSocket
                Ok(false)
            }
        }
    }
}

/// No-op BranchManager for testing or when branching is disabled.
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
    use crate::domain::{
        AgentId, BranchConfig, BranchId, BranchStatus, MergeRequestId, MergeStatus, Priority, Task,
        TaskId, TaskStatus,
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
        next_merge_id: RwLock<u64>,
    }

    impl MockBranchManager {
        fn new() -> Self {
            Self {
                branches: RwLock::new(HashMap::new()),
                merges: RwLock::new(HashMap::new()),
                next_branch_id: RwLock::new(1),
                next_merge_id: RwLock::new(1),
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
            let mut id_ref = self.next_merge_id.write().unwrap();
            let id = MergeRequestId::new(*id_ref);
            *id_ref += 1;
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
            Some(BranchingStrategy::MultipleReviewers)
        );

        let task2 = test_task_with_content(
            2,
            "Implement both approaches for A/B testing",
            TaskStatus::Pending,
        );
        assert_eq!(
            crate::domain::should_use_branching(&task2),
            Some(BranchingStrategy::AlternativeImplementations)
        );

        let task3 = test_task_with_content(
            3,
            "Refactor codebase with sub-tasks for each module",
            TaskStatus::Pending,
        );
        assert_eq!(
            crate::domain::should_use_branching(&task3),
            Some(BranchingStrategy::NestedBranches)
        );

        let task4 = test_task_with_content(4, "Simple bug fix", TaskStatus::Pending);
        assert_eq!(crate::domain::should_use_branching(&task4), None);
    }
}
