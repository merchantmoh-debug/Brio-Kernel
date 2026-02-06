//! Handler for the Pending task state.
//!
//! Transitions pending tasks to the planning state.

use super::{SupervisorContext, TaskStateHandler};
use crate::domain::{Task, TaskStatus};
use crate::mesh_client::AgentDispatcher;
use crate::orchestrator::{Planner, SupervisorError};
use crate::repository::TaskRepository;
use crate::selector::AgentSelector;

/// Handler for pending tasks awaiting planning.
pub struct PendingHandler;

impl<R, D, P, S> TaskStateHandler<R, D, P, S> for PendingHandler
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
        ctx.repository
            .update_status(task.id(), TaskStatus::Planning)
            .map_err(SupervisorError::StatusUpdateFailure)?;
        Ok(true)
    }
}
