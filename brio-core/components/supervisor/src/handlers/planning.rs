//! Handler for the Planning task state.
//!
//! Decomposes tasks into subtasks using the planner.

use super::{SupervisorContext, TaskStateHandler};
use crate::domain::{Priority, Task, TaskStatus};
use crate::mesh_client::AgentDispatcher;
use crate::orchestrator::{Planner, SupervisorError};
use crate::repository::TaskRepository;
use crate::selector::AgentSelector;

/// Handler for planning and decomposing tasks.
pub struct PlanningHandler;

impl<R, D, P, S> TaskStateHandler<R, D, P, S> for PlanningHandler
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
        let plan_result = ctx
            .planner
            .plan(task.content())
            .map_err(SupervisorError::PlanningFailure)?;

        match plan_result {
            Some(subtasks) if !subtasks.is_empty() => {
                for sub_content in subtasks {
                    ctx.repository
                        .create_task(sub_content, Priority::DEFAULT, Some(task.id()))
                        .map_err(SupervisorError::RepositoryFailure)?;
                }

                ctx.repository
                    .update_status(task.id(), TaskStatus::Coordinating)
                    .map_err(SupervisorError::StatusUpdateFailure)?;
            }
            _ => {
                ctx.repository
                    .update_status(task.id(), TaskStatus::Executing)
                    .map_err(SupervisorError::StatusUpdateFailure)?;
            }
        }
        Ok(true)
    }
}
