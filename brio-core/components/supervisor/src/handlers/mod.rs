//! State handlers for the Supervisor task lifecycle.
//!
//! Each handler implements `TaskStateHandler` for a specific `TaskStatus`.

use crate::domain::Task;
use crate::mesh_client::AgentDispatcher;
use crate::orchestrator::{Planner, SupervisorError};
use crate::repository::TaskRepository;
use crate::selector::AgentSelector;

/// Context passed to state handlers containing all necessary dependencies.
pub struct SupervisorContext<'a, R, D, P, S> {
    /// Repository for task persistence.
    pub repository: &'a R,
    /// Dispatcher for agent communication.
    pub dispatcher: &'a D,
    /// Planner for task decomposition.
    pub planner: &'a P,
    /// Selector for agent assignment.
    pub selector: &'a S,
}

/// Trait representing a handler for a specific task state.
pub trait TaskStateHandler<R, D, P, S>
where
    R: TaskRepository,
    D: AgentDispatcher,
    P: Planner,
    S: AgentSelector,
{
    /// Processes a task in the current state.
    ///
    /// Returns `true` if a state transition or significant action occurred, `false` otherwise.
    ///
    /// # Errors
    /// Returns `SupervisorError` if processing fails.
    fn handle(
        &self,
        ctx: &SupervisorContext<R, D, P, S>,
        task: &Task,
    ) -> Result<bool, SupervisorError>;
}

pub mod coordinating;
pub mod executing;
pub mod pending;
pub mod planning;
pub mod verifying;
