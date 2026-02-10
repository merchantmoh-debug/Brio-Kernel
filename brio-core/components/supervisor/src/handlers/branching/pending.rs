//! Handler for `MergePendingApproval` state.
//!
//! Waits for external approval via REST API or WebSocket command.

use crate::domain::{MergeStatus, Task, TaskStatus};
use crate::handlers::{SupervisorContext, TaskStateHandler};
use crate::orchestrator::{Planner, SupervisorError};
use crate::repository::TaskRepository;

/// Handler for `MergePendingApproval` state.
///
/// Waits for external approval via REST API or WebSocket command.
pub struct MergePendingApprovalHandler;

impl<R, D, P, S> TaskStateHandler<R, D, P, S> for MergePendingApprovalHandler
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
        let merge_status = branch_manager.get_merge_status(merge_request_id).map_err(
            |e: crate::handlers::branching::BranchManagerError| {
                SupervisorError::RepositoryFailure(crate::repository::RepositoryError::SqlError(
                    e.to_string(),
                ))
            },
        )?;

        match merge_status {
            MergeStatus::Approved => {
                // Merge was approved - execute it
                branch_manager.execute_merge(merge_request_id).map_err(
                    |e: crate::handlers::branching::BranchManagerError| {
                        SupervisorError::RepositoryFailure(
                            crate::repository::RepositoryError::SqlError(e.to_string()),
                        )
                    },
                )?;

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
