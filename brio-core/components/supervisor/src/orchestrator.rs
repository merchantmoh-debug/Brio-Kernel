//! Orchestrator Layer - Supervisor Business Logic
//!
//! This module contains the core supervision loop that:
//! 1. Queries pending tasks from the repository
//! 2. Dispatches each to an appropriate agent
//! 3. Updates task status based on dispatch result
//!
//! Dependencies are injected via traits (DIP), enabling testability.

use crate::domain::{AgentId, Task, TaskStatus};
use crate::mesh_client::{AgentDispatcher, DispatchResult, MeshError};
use crate::repository::{RepositoryError, TaskRepository};

// =============================================================================
// Planner Trait
// =============================================================================

/// Task decomposition capability.
pub trait Planner {
    /// Decomposes a task into subtasks or a plan.
    ///
    /// # Errors
    /// Returns error if planning fails.
    fn plan(&self, objective: &str) -> Result<(), PlannerError>;
}

/// Errors occurring during planning.
#[derive(Debug)]
pub struct PlannerError(pub String);

impl core::fmt::Display for PlannerError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Planning error: {}", self.0)
    }
}

// =============================================================================
// Error Types
// =============================================================================

/// Errors that can occur during supervision.
#[derive(Debug)]
pub enum SupervisorError {
    /// Failed to fetch tasks from repository.
    RepositoryFailure(RepositoryError),
    /// Failed to dispatch task to agent.
    DispatchFailure(MeshError),
    /// Failed to update task status.
    StatusUpdateFailure(RepositoryError),
    /// Failed to plan task.
    PlanningFailure(PlannerError),
}

impl core::fmt::Display for SupervisorError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::RepositoryFailure(e) => write!(f, "Repository failure: {e}"),
            Self::DispatchFailure(e) => write!(f, "Dispatch failure: {e}"),
            Self::StatusUpdateFailure(e) => write!(f, "Status update failure: {e}"),
            Self::PlanningFailure(e) => write!(f, "Planning failure: {e}"),
        }
    }
}

impl std::error::Error for SupervisorError {}

impl From<RepositoryError> for SupervisorError {
    fn from(e: RepositoryError) -> Self {
        Self::RepositoryFailure(e)
    }
}

impl From<MeshError> for SupervisorError {
    fn from(e: MeshError) -> Self {
        Self::DispatchFailure(e)
    }
}

// =============================================================================
// Supervisor
// =============================================================================

/// Supervisor orchestration logic.
///
/// Coordinates task fetching, agent dispatch, and status updates.
/// Dependencies are injected via generic trait bounds.
pub struct Supervisor<R, D, P>
where
    R: TaskRepository,
    D: AgentDispatcher,
    P: Planner,
{
    repository: R,
    dispatcher: D,
    planner: P,
}

impl<R, D, P> Supervisor<R, D, P>
where
    R: TaskRepository,
    D: AgentDispatcher,
    P: Planner,
{
    /// Creates a new Supervisor with injected dependencies.
    #[must_use]
    pub const fn new(repository: R, dispatcher: D, planner: P) -> Self {
        Self {
            repository,
            dispatcher,
            planner,
        }
    }

    /// Executes a single poll cycle.
    ///
    /// Fetches all pending tasks and attempts to dispatch each one.
    /// Returns the count of successfully dispatched tasks.
    ///
    /// # Errors
    /// Returns `SupervisorError` if any critical operation fails.
    pub fn poll_tasks(&self) -> Result<u32, SupervisorError> {
        let active_tasks = self
            .repository
            .fetch_active_tasks()
            .map_err(SupervisorError::RepositoryFailure)?;
        let mut processed_count: u32 = 0;

        for task in active_tasks {
            match self.process_task(&task) {
                Ok(true) => processed_count += 1,
                Ok(false) => { /* Task checked but no state transition occurred */ }
                Err(e) => {
                    self.handle_failure(&task, &e);
                }
            }
        }

        Ok(processed_count)
    }

    /// Processes a single task based on its current state.
    fn process_task(&self, task: &Task) -> Result<bool, SupervisorError> {
        match task.status() {
            TaskStatus::Pending => {
                // Pending -> Planning
                self.repository
                    .update_status(task.id(), TaskStatus::Planning)
                    .map_err(SupervisorError::StatusUpdateFailure)?;
                Ok(true)
            }
            TaskStatus::Planning => {
                // Planning -> Executing
                self.planner
                    .plan(task.content())
                    .map_err(SupervisorError::PlanningFailure)?;

                self.repository
                    .update_status(task.id(), TaskStatus::Executing)
                    .map_err(SupervisorError::StatusUpdateFailure)?;
                Ok(true)
            }
            TaskStatus::Executing => {
                // Executing -> Assigned (Dispatch)
                // We dispatch if not already assigned.
                if task.assigned_agent().is_some() {
                    // Already assigned. Waiting for external completion (or Agent to update status).
                    // For now, we do nothing and return false to indicate no state transition by Supervisor.
                    return Ok(false);
                }

                let agent = self.select_agent(task);
                match self.dispatcher.dispatch(&agent, task)? {
                    DispatchResult::Accepted => {
                        // Mark as assigned but KEEP status as Executing so we stay in the active loop
                        // (or relies on Agent to move it to Verifying/Completed)
                        self.repository
                            .assign_agent(task.id(), &agent)
                            .map_err(SupervisorError::StatusUpdateFailure)?;
                        Ok(true)
                    }
                    DispatchResult::AgentBusy => Ok(false),
                }
            }
            TaskStatus::Verifying => {
                // Verifying -> Completed (Placeholder)
                self.repository
                    .mark_completed(task.id())
                    .map_err(SupervisorError::StatusUpdateFailure)?;
                Ok(true)
            }
            _ => Ok(false), // Ignore other states
        }
    }

    /// Selects an appropriate agent for the task.
    ///
    /// Current implementation: simple default agent selection.
    /// Future: Could use task metadata or load balancing.
    fn select_agent(&self, _task: &Task) -> AgentId {
        // Default agent for MVP
        AgentId::new("agent_coder")
    }

    /// Handles failures during task dispatch.
    fn handle_failure(&self, task: &Task, error: &SupervisorError) {
        let reason = error.to_string();
        // Best-effort status update; ignore secondary failures
        let _ = self.repository.mark_failed(task.id(), &reason);
    }
}

// =============================================================================
// Unit Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{Priority, TaskId, TaskStatus};
    use std::cell::RefCell;
    use std::rc::Rc;

    /// Shared mock repository state.
    struct MockRepositoryInner {
        tasks: Vec<Task>,
        assigned: RefCell<Vec<(TaskId, AgentId)>>,
        completed: RefCell<Vec<TaskId>>,
        failed: RefCell<Vec<(TaskId, String)>>,
    }

    /// Mock repository for testing (reference-counted for safe sharing).
    #[derive(Clone)]
    struct MockRepository(Rc<MockRepositoryInner>);

    impl MockRepository {
        fn new(tasks: Vec<Task>) -> Self {
            Self(Rc::new(MockRepositoryInner {
                tasks,
                assigned: RefCell::new(vec![]),
                completed: RefCell::new(vec![]),
                failed: RefCell::new(vec![]),
            }))
        }

        fn assigned(&self) -> std::cell::Ref<'_, Vec<(TaskId, AgentId)>> {
            self.0.assigned.borrow()
        }
    }

    impl TaskRepository for MockRepository {
        fn fetch_active_tasks(&self) -> Result<Vec<Task>, RepositoryError> {
            Ok(self
                .0
                .tasks
                .iter()
                .filter(|t| t.is_active())
                .cloned()
                .collect())
        }

        fn update_status(
            &self,
            _task_id: TaskId,
            _status: TaskStatus,
        ) -> Result<(), RepositoryError> {
            // Mock implementation: success (state change not persisted in this simple mock)
            Ok(())
        }

        fn assign_agent(&self, task_id: TaskId, agent: &AgentId) -> Result<(), RepositoryError> {
            self.0.assigned.borrow_mut().push((task_id, agent.clone()));
            Ok(())
        }

        fn mark_assigned(&self, task_id: TaskId, agent: &AgentId) -> Result<(), RepositoryError> {
            self.0.assigned.borrow_mut().push((task_id, agent.clone()));
            Ok(())
        }

        fn mark_completed(&self, task_id: TaskId) -> Result<(), RepositoryError> {
            self.0.completed.borrow_mut().push(task_id);
            Ok(())
        }

        fn mark_failed(&self, task_id: TaskId, reason: &str) -> Result<(), RepositoryError> {
            self.0
                .failed
                .borrow_mut()
                .push((task_id, reason.to_string()));
            Ok(())
        }
    }

    struct MockPlanner;
    impl Planner for MockPlanner {
        fn plan(&self, _objective: &str) -> Result<(), PlannerError> {
            Ok(())
        }
    }

    /// Mock dispatcher for testing.
    struct MockDispatcher {
        result: DispatchResult,
    }

    impl AgentDispatcher for MockDispatcher {
        fn dispatch(&self, _agent: &AgentId, _task: &Task) -> Result<DispatchResult, MeshError> {
            Ok(self.result.clone())
        }
    }

    fn test_task(id: u64, content: &str) -> Task {
        Task::new(
            TaskId::new(id),
            content.to_string(),
            Priority::DEFAULT,
            TaskStatus::Pending,
            None,
        )
    }

    #[test]
    fn poll_dispatches_pending_tasks() {
        let repo = MockRepository::new(vec![test_task(1, "task1"), test_task(2, "task2")]);
        let planner = MockPlanner;
        let dispatcher = MockDispatcher {
            result: DispatchResult::Accepted,
        };

        let supervisor = Supervisor::new(repo, dispatcher, planner);
        let count = supervisor.poll_tasks().unwrap();

        // Pending -> Planning (2 tasks processed)
        assert_eq!(count, 2);
    }

    #[test]
    fn poll_executing_tasks_logic() {
        // Test transition from Executing -> Assigned
        let task = Task::new(
            TaskId::new(3),
            "task3".to_string(),
            Priority::DEFAULT,
            TaskStatus::Executing,
            None,
        );
        let repo = MockRepository::new(vec![task]);
        let planner = MockPlanner;
        let dispatcher = MockDispatcher {
            result: DispatchResult::Accepted,
        };

        let supervisor = Supervisor::new(repo, dispatcher, planner);
        let count = supervisor.poll_tasks().unwrap();

        // 1 active task processed (dispatched and assigned)
        assert_eq!(count, 1);
    }

    #[test]
    fn poll_executing_tasks_already_assigned() {
        // If already assigned, should not re-dispatch
        let mut task = Task::new(
            TaskId::new(3),
            "task3".to_string(),
            Priority::DEFAULT,
            TaskStatus::Executing,
            None,
        );
        // Manually assign
        task = Task::new(
            task.id(),
            task.content().to_string(),
            task.priority(),
            task.status(),
            Some(AgentId::new("agent_coder")),
        );

        let repo = MockRepository::new(vec![task]);
        let planner = MockPlanner;
        let dispatcher = MockDispatcher {
            result: DispatchResult::Accepted,
        };

        let supervisor = Supervisor::new(repo, dispatcher, planner);
        let count = supervisor.poll_tasks().unwrap();

        // No processing/changes (count = 0)
        assert_eq!(count, 0);
    }

    #[test]
    fn poll_marks_assigned_on_accept() {
        // Need to be in Executing state to dispatch
        let task = Task::new(
            TaskId::new(42),
            "task42".to_string(),
            Priority::DEFAULT,
            TaskStatus::Executing,
            None,
        );
        let repo = MockRepository::new(vec![task]);
        let repo_clone = repo.clone();
        let planner = MockPlanner;

        let dispatcher = MockDispatcher {
            result: DispatchResult::Accepted,
        };

        let supervisor = Supervisor::new(repo, dispatcher, planner);
        supervisor.poll_tasks().unwrap();

        let assigned = repo_clone.assigned();
        assert_eq!(assigned.len(), 1);
        assert_eq!(assigned[0].0.inner(), 42);
    }
}
