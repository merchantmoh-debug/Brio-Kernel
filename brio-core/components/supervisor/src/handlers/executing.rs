//! Handler for the Executing task state.
//!
//! Dispatches tasks to agents for execution.

use super::{SupervisorContext, TaskStateHandler};
use crate::domain::Task;
use crate::mesh_client::{AgentDispatcher, DispatchResult};
use crate::orchestrator::{Planner, SupervisorError};
use crate::repository::TaskRepository;
use crate::selector::AgentSelector;

/// Handler for dispatching tasks to agents.
pub struct ExecutingHandler;

impl<R, D, P, S> TaskStateHandler<R, D, P, S> for ExecutingHandler
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
        if task.assigned_agent().is_some() {
            return Ok(false);
        }

        let agent = ctx.selector.select(task);
        match ctx.dispatcher.dispatch(&agent, task)? {
            DispatchResult::Accepted => {
                ctx.repository
                    .assign_agent(task.id(), &agent)
                    .map_err(SupervisorError::StatusUpdateFailure)?;
                Ok(true)
            }
            DispatchResult::Completed(_) => {
                ctx.repository
                    .mark_completed(task.id())
                    .map_err(SupervisorError::StatusUpdateFailure)?;
                Ok(true)
            }
            DispatchResult::AgentBusy => Ok(false),
        }
    }
}
