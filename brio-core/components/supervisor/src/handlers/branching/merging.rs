//! Handler for Merging state.
//!
//! Monitors merge operations and completes tasks or handles conflicts.

use crate::domain::{MergeStatus, Task, TaskStatus};
use crate::handlers::{BranchManager, SupervisorContext, TaskStateHandler};
use crate::orchestrator::{Planner, SupervisorError};
use crate::repository::TaskRepository;

/// Handler for Merging state.
///
/// Monitors merge operations and completes tasks or handles conflicts.
pub struct MergingHandler;

impl<R, D, P, S> TaskStateHandler<R, D, P, S> for MergingHandler
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
        let merge_status = branch_manager.get_merge_status(merge_request_id).map_err(
            |e: crate::handlers::branching::BranchManagerError| {
                SupervisorError::RepositoryFailure(crate::repository::RepositoryError::SqlError(
                    e.to_string(),
                ))
            },
        )?;

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
