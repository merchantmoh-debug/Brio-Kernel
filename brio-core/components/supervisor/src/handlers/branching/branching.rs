//! Handler for Branching state.
//!
//! Monitors branch execution progress and transitions to Merging or
//! `MergePendingApproval` when branches complete.

use crate::domain::{BranchStatus, Task, TaskStatus};
use crate::handlers::branching::merge_strategy;
use crate::handlers::{SupervisorContext, TaskStateHandler};
use crate::orchestrator::{Planner, SupervisorError};
use crate::repository::TaskRepository;

/// Handler for Branching state.
///
/// Monitors branch execution progress and transitions to Merging or
/// `MergePendingApproval` when branches complete.
pub struct BranchingHandler;

impl<R, D, P, S> TaskStateHandler<R, D, P, S> for BranchingHandler
where
    R: TaskRepository,
    D: crate::mesh_client::AgentDispatcher,
    P: Planner,
    S: crate::selector::AgentSelector,
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
            SupervisorError::RepositoryFailure(crate::repository::RepositoryError::SqlError(
                e.to_string(),
            ))
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
                .create_merge_request(&branches, merge_strategy::UNION, requires_approval)
                .map_err(|e| {
                    SupervisorError::RepositoryFailure(
                        crate::repository::RepositoryError::SqlError(e.to_string()),
                    )
                })?;

            if requires_approval {
                // Get conflicts for display
                let conflicts = branch_manager
                    .get_merge_conflicts(merge_request_id)
                    .map_err(|e: crate::handlers::branching::BranchManagerError| {
                        SupervisorError::RepositoryFailure(
                            crate::repository::RepositoryError::SqlError(e.to_string()),
                        )
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
