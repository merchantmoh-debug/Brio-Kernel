//! Handler for the Verifying task state.
//!
//! Marks tasks as completed after verification.

use super::{SupervisorContext, TaskStateHandler};
use crate::domain::Task;
use crate::mesh_client::AgentDispatcher;
use crate::orchestrator::{Planner, SupervisorError};
use crate::repository::TaskRepository;
use crate::selector::AgentSelector;

/// Handler for verifying and completing tasks.
pub struct VerifyingHandler;

impl<R, D, P, S> TaskStateHandler<R, D, P, S> for VerifyingHandler
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
            .mark_completed(task.id())
            .map_err(SupervisorError::StatusUpdateFailure)?;
        Ok(true)
    }
}
