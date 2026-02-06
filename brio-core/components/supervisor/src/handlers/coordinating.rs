//! Handler for the Coordinating task state.
//!
//! Manages tasks that are coordinating between multiple subtasks.

use super::{SupervisorContext, TaskStateHandler};
use crate::domain::{Task, TaskStatus};
use crate::mesh_client::AgentDispatcher;
use crate::orchestrator::{Planner, SupervisorError};
use crate::repository::TaskRepository;
use crate::selector::AgentSelector;

/// Handler for coordinating tasks between subtasks.
pub struct CoordinatingHandler;

impl<R, D, P, S> TaskStateHandler<R, D, P, S> for CoordinatingHandler
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
        let subtasks = ctx
            .repository
            .fetch_subtasks(task.id())
            .map_err(SupervisorError::RepositoryFailure)?;

        if subtasks.is_empty() {
            ctx.repository
                .update_status(task.id(), TaskStatus::Verifying)
                .map_err(SupervisorError::StatusUpdateFailure)?;
            return Ok(true);
        }

        if subtasks
            .iter()
            .any(|t| matches!(t.status(), TaskStatus::Failed))
        {
            ctx.repository
                .mark_failed(task.id(), "Subtask failed")
                .map_err(SupervisorError::StatusUpdateFailure)?;
            return Ok(true);
        }

        if subtasks
            .iter()
            .all(|t| matches!(t.status(), TaskStatus::Completed))
        {
            ctx.repository
                .update_status(task.id(), TaskStatus::Verifying)
                .map_err(SupervisorError::StatusUpdateFailure)?;
            return Ok(true);
        }

        Ok(false)
    }
}
