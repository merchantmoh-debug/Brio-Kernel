//! Orchestrator Layer - Supervisor Business Logic
//!
//! This module contains the core supervision loop that:
//! 1. Queries pending tasks from the repository
//! 2. Dispatches each to an appropriate agent
//! 3. Updates task status based on dispatch result
//!
//! Dependencies are injected via traits (DIP), enabling testability.

use crate::domain::{Task, TaskStatus};
use crate::mesh_client::{AgentDispatcher, MeshError};
use crate::repository::{RepositoryError, TaskRepository};
use crate::selector::AgentSelector;

use crate::handlers;

/// Task decomposition capability.
pub trait Planner {
    /// Decomposes a task into subtasks or a plan.
    ///
    /// # Errors
    /// Returns error if planning fails.
    fn plan(&self, objective: &str) -> Result<Option<Vec<String>>, PlannerError>;
}

/// Errors occurring during planning.
#[derive(Debug)]
pub struct PlannerError(pub String);

impl core::fmt::Display for PlannerError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Planning error: {}", self.0)
    }
}

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

/// Coordinates task fetching, agent dispatch, and status updates.
/// Dependencies are injected via generic trait bounds.
pub struct Supervisor<R, D, P, S>
where
    R: TaskRepository,
    D: AgentDispatcher,
    P: Planner,
    S: AgentSelector,
{
    pub(crate) repository: R,
    pub(crate) dispatcher: D,
    pub(crate) planner: P,
    pub(crate) selector: S,
}

impl<R, D, P, S> Supervisor<R, D, P, S>
where
    R: TaskRepository,
    D: AgentDispatcher,
    P: Planner,
    S: AgentSelector,
{
    /// Creates a new Supervisor with injected dependencies.
    #[must_use]
    pub const fn new(repository: R, dispatcher: D, planner: P, selector: S) -> Self {
        Self {
            repository,
            dispatcher,
            planner,
            selector,
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

        let ctx = handlers::SupervisorContext {
            repository: &self.repository,
            dispatcher: &self.dispatcher,
            planner: &self.planner,
            selector: &self.selector,
        };

        for task in active_tasks {
            match Self::process_task_with_handlers(&ctx, &task) {
                Ok(true) => processed_count += 1,
                Ok(false) => { /* Task checked but no state transition occurred */ }
                Err(e) => {
                    self.handle_failure(&task, &e);
                }
            }
        }

        Ok(processed_count)
    }

    fn process_task_with_handlers(
        ctx: &handlers::SupervisorContext<R, D, P, S>,
        task: &Task,
    ) -> Result<bool, SupervisorError> {
        use handlers::TaskStateHandler;
        match task.status() {
            TaskStatus::Pending => handlers::pending::PendingHandler.handle(ctx, task),
            TaskStatus::Planning => handlers::planning::PlanningHandler.handle(ctx, task),
            TaskStatus::Executing => handlers::executing::ExecutingHandler.handle(ctx, task),
            TaskStatus::Coordinating => {
                handlers::coordinating::CoordinatingHandler.handle(ctx, task)
            }
            TaskStatus::Verifying => handlers::verifying::VerifyingHandler.handle(ctx, task),
            _ => Ok(false),
        }
    }

    /// Handles failures during task dispatch.
    fn handle_failure(&self, task: &Task, error: &SupervisorError) {
        let reason = error.to_string();
        // Best-effort status update; ignore secondary failures but log them
        if let Err(e) = self.repository.mark_failed(task.id(), &reason) {
            eprintln!(
                "[supervisor] Failed to mark task {} as failed: {}",
                task.id(),
                e
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{AgentId, Priority, TaskId, TaskStatus};
    use crate::mesh_client::{DispatchResult, MeshError};
    use crate::selector::KeywordAgentSelector;
    use std::cell::RefCell;
    use std::collections::HashSet;
    use std::rc::Rc;

    /// Shared mock repository state.
    struct MockRepositoryInner {
        tasks: RefCell<Vec<Task>>,
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
                tasks: RefCell::new(tasks),
                assigned: RefCell::new(vec![]),
                completed: RefCell::new(vec![]),
                failed: RefCell::new(vec![]),
            }))
        }

        fn task(&self, id: TaskId) -> Option<Task> {
            self.0.tasks.borrow().iter().find(|t| t.id() == id).cloned()
        }

        // This method is part of the test API for the mock store.
        // It's currently unused but preserved for test extensibility.
        #[expect(dead_code)]
        fn add_task(&self, task: Task) {
            let mut tasks = self.0.tasks.borrow_mut();
            if let Some(pos) = tasks.iter().position(|t| t.id() == task.id()) {
                tasks[pos] = task;
            } else {
                tasks.push(task);
            }
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
                .borrow()
                .iter()
                .filter(|t| t.is_active())
                .cloned()
                .collect())
        }

        fn update_status(
            &self,
            task_id: TaskId,
            status: TaskStatus,
        ) -> Result<(), RepositoryError> {
            let mut tasks = self.0.tasks.borrow_mut();
            if let Some(t) = tasks.iter_mut().find(|t| t.id() == task_id) {
                *t = Task::new(
                    t.id(),
                    t.content().to_string(),
                    t.priority(),
                    status,
                    t.parent_id(),
                    t.assigned_agent().cloned(),
                    t.required_capabilities().clone(),
                )
                .map_err(|e| RepositoryError::ParseError(e.to_string()))?;
            }
            Ok(())
        }

        fn assign_agent(&self, task_id: TaskId, agent: &AgentId) -> Result<(), RepositoryError> {
            self.0.assigned.borrow_mut().push((task_id, agent.clone()));

            let mut tasks = self.0.tasks.borrow_mut();
            if let Some(t) = tasks.iter_mut().find(|t| t.id() == task_id) {
                *t = Task::new(
                    t.id(),
                    t.content().to_string(),
                    t.priority(),
                    t.status(),
                    t.parent_id(),
                    Some(agent.clone()),
                    t.required_capabilities().clone(),
                )
                .map_err(|e| RepositoryError::ParseError(e.to_string()))?;
            }
            Ok(())
        }

        fn mark_assigned(&self, task_id: TaskId, agent: &AgentId) -> Result<(), RepositoryError> {
            self.0.assigned.borrow_mut().push((task_id, agent.clone()));

            let mut tasks = self.0.tasks.borrow_mut();
            if let Some(t) = tasks.iter_mut().find(|t| t.id() == task_id) {
                *t = Task::new(
                    t.id(),
                    t.content().to_string(),
                    t.priority(),
                    TaskStatus::Assigned,
                    t.parent_id(),
                    Some(agent.clone()),
                    t.required_capabilities().clone(),
                )
                .map_err(|e| RepositoryError::ParseError(e.to_string()))?;
            }
            Ok(())
        }

        fn mark_completed(&self, task_id: TaskId) -> Result<(), RepositoryError> {
            self.0.completed.borrow_mut().push(task_id);

            let mut tasks = self.0.tasks.borrow_mut();
            if let Some(t) = tasks.iter_mut().find(|t| t.id() == task_id) {
                *t = Task::new(
                    t.id(),
                    t.content().to_string(),
                    t.priority(),
                    TaskStatus::Completed,
                    t.parent_id(),
                    t.assigned_agent().cloned(),
                    t.required_capabilities().clone(),
                )
                .map_err(|e| RepositoryError::ParseError(e.to_string()))?;
            }
            Ok(())
        }

        fn mark_failed(&self, task_id: TaskId, reason: &str) -> Result<(), RepositoryError> {
            self.0
                .failed
                .borrow_mut()
                .push((task_id, reason.to_string()));

            // Also update status in main list so fetch_subtasks works correctly
            let mut tasks = self.0.tasks.borrow_mut();
            if let Some(t) = tasks.iter_mut().find(|t| t.id() == task_id) {
                *t = Task::new(
                    t.id(),
                    t.content().to_string(),
                    t.priority(),
                    TaskStatus::Failed,
                    t.parent_id(),
                    t.assigned_agent().cloned(),
                    t.required_capabilities().clone(),
                )
                .map_err(|e| RepositoryError::ParseError(e.to_string()))?;
            }
            Ok(())
        }

        fn create_task(
            &self,
            content: String,
            priority: Priority,
            parent_id: Option<TaskId>,
        ) -> Result<TaskId, RepositoryError> {
            let mut tasks = self.0.tasks.borrow_mut();
            // Simple auto-increment
            let max_id = tasks.iter().map(|t| t.id().inner()).max().unwrap_or(0);
            let new_id = TaskId::new(max_id + 1);
            let task = Task::new(
                new_id,
                content,
                priority,
                TaskStatus::Pending,
                parent_id,
                None,
                HashSet::new(),
            )
            .map_err(|e| RepositoryError::ParseError(e.to_string()))?;
            tasks.push(task);
            Ok(new_id)
        }

        fn fetch_subtasks(&self, parent_id: TaskId) -> Result<Vec<Task>, RepositoryError> {
            Ok(self
                .0
                .tasks
                .borrow()
                .iter()
                .filter(|t| t.parent_id() == Some(parent_id))
                .cloned()
                .collect())
        }
    }

    struct MockPlanner;
    impl Planner for MockPlanner {
        fn plan(&self, _objective: &str) -> Result<Option<Vec<String>>, PlannerError> {
            // Default: no decomposition
            Ok(None)
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
            None,
            HashSet::new(),
        )
        .expect("test task should be valid")
    }

    #[test]
    fn poll_dispatches_pending_tasks() -> Result<(), SupervisorError> {
        let repo = MockRepository::new(vec![test_task(1, "task1"), test_task(2, "task2")]);
        let planner = MockPlanner;
        let dispatcher = MockDispatcher {
            result: DispatchResult::Accepted,
        };
        let selector = KeywordAgentSelector;

        let supervisor = Supervisor::new(repo, dispatcher, planner, selector);
        let count = supervisor.poll_tasks()?;

        // Pending -> Planning (2 tasks processed)
        assert_eq!(count, 2);
        Ok(())
    }

    #[test]
    fn poll_executing_tasks_logic() -> Result<(), SupervisorError> {
        // Test transition from Executing -> Assigned
        let task = Task::new(
            TaskId::new(3),
            "task3".to_string(),
            Priority::DEFAULT,
            TaskStatus::Executing,
            None,
            None,
            HashSet::new(),
        )
        .expect("test task should be valid");
        let repo = MockRepository::new(vec![task]);
        let planner = MockPlanner;
        let dispatcher = MockDispatcher {
            result: DispatchResult::Accepted,
        };
        let selector = KeywordAgentSelector;

        let supervisor = Supervisor::new(repo, dispatcher, planner, selector);
        let count = supervisor.poll_tasks()?;

        // 1 active task processed (dispatched and assigned)
        assert_eq!(count, 1);
        Ok(())
    }

    #[test]
    fn poll_executing_tasks_already_assigned() -> Result<(), SupervisorError> {
        // If already assigned, should not re-dispatch
        let mut task = Task::new(
            TaskId::new(3),
            "task3".to_string(),
            Priority::DEFAULT,
            TaskStatus::Executing,
            None,
            None,
            HashSet::new(),
        )
        .expect("test task should be valid");
        // Manually assign
        task = Task::new(
            task.id(),
            task.content().to_string(),
            task.priority(),
            task.status(),
            task.parent_id(),
            Some(AgentId::new("agent_coder").expect("agent ID should be valid")),
            HashSet::new(),
        )
        .expect("test task should be valid");

        let repo = MockRepository::new(vec![task]);
        let planner = MockPlanner;
        let dispatcher = MockDispatcher {
            result: DispatchResult::Accepted,
        };
        let selector = KeywordAgentSelector;

        let supervisor = Supervisor::new(repo, dispatcher, planner, selector);
        let count = supervisor.poll_tasks()?;

        // No processing/changes (count = 0)
        assert_eq!(count, 0);
        Ok(())
    }

    #[test]
    fn poll_marks_assigned_on_accept() -> Result<(), SupervisorError> {
        // Need to be in Executing state to dispatch
        let task = Task::new(
            TaskId::new(42),
            "task42".to_string(),
            Priority::DEFAULT,
            TaskStatus::Executing,
            None,
            None,
            HashSet::new(),
        )
        .expect("test task should be valid");
        let repo = MockRepository::new(vec![task]);
        let repo_clone = repo.clone();
        let planner = MockPlanner;

        let dispatcher = MockDispatcher {
            result: DispatchResult::Accepted,
        };
        let selector = KeywordAgentSelector;

        let supervisor = Supervisor::new(repo, dispatcher, planner, selector);
        supervisor.poll_tasks()?;

        let assigned = repo_clone.assigned();
        assert_eq!(assigned.len(), 1);
        assert_eq!(assigned[0].0.inner(), 42);
        Ok(())
    }

    struct DecomposingPlanner {
        subtasks: Vec<String>,
    }

    impl Planner for DecomposingPlanner {
        fn plan(&self, _objective: &str) -> Result<Option<Vec<String>>, PlannerError> {
            Ok(Some(self.subtasks.clone()))
        }
    }

    #[test]
    fn test_task_decomposition_flow() -> Result<(), SupervisorError> {
        // Setup: 1 pending task, Planner returns 2 subtasks
        let root_task = Task::new(
            TaskId::new(1),
            "Build Death Star".to_string(),
            Priority::DEFAULT,
            TaskStatus::Pending,
            None,
            None,
            HashSet::new(),
        )
        .expect("test task should be valid");
        let repo = MockRepository::new(vec![root_task]);
        let planner = DecomposingPlanner {
            subtasks: vec!["Get Plans".to_string(), "Find Weakness".to_string()],
        };
        let dispatcher = MockDispatcher {
            result: DispatchResult::Accepted,
        };
        let selector = KeywordAgentSelector;

        let supervisor = Supervisor::new(repo.clone(), dispatcher, planner, selector);

        // 1. Poll: Pending -> Planning
        let count = supervisor.poll_tasks()?;
        assert_eq!(count, 1);
        let t1 = repo.task(TaskId::new(1)).expect("Task 1 should exist");
        assert_eq!(t1.status(), TaskStatus::Planning);

        // 2. Poll: Planning -> Coordinating + Subtasks Created
        let count = supervisor.poll_tasks()?;
        assert_eq!(count, 1);
        let t1 = repo.task(TaskId::new(1)).expect("Task 1 should exist");
        assert_eq!(t1.status(), TaskStatus::Coordinating);

        // Verify subtasks created
        let subtasks = repo.fetch_subtasks(TaskId::new(1))?;
        assert_eq!(subtasks.len(), 2);
        assert_eq!(subtasks[0].content(), "Get Plans");
        assert_eq!(subtasks[0].status(), TaskStatus::Pending);

        // 3. Poll: Coordinating -> checks subtasks (all Pending/Executing) -> Stays Coordinating

        let count = supervisor.poll_tasks()?;
        assert_eq!(count, 2);

        // 4. Manually complete subtasks to test completion flow
        repo.update_status(TaskId::new(2), TaskStatus::Completed)?;
        repo.update_status(TaskId::new(3), TaskStatus::Completed)?;

        // 5. Poll: Root checks logic.
        let count = supervisor.poll_tasks()?;
        assert_eq!(count, 1);

        let t1 = repo.task(TaskId::new(1)).expect("Task 1 should exist");
        assert_eq!(t1.status(), TaskStatus::Verifying);
        Ok(())
    }

    #[test]
    fn select_agent_reviewer_based_on_keyword() {
        let task = Task::new(
            TaskId::new(100),
            "Please review this code".to_string(),
            Priority::DEFAULT,
            TaskStatus::Executing,
            None,
            None,
            HashSet::new(),
        )
        .expect("test task should be valid");

        let repo = MockRepository::new(vec![task.clone()]);
        let planner = MockPlanner;
        let dispatcher = MockDispatcher {
            result: DispatchResult::Accepted,
        };
        let selector = KeywordAgentSelector;

        let supervisor = Supervisor::new(repo, dispatcher, planner, selector);
        let agent_id = supervisor.selector.select(&task);

        assert_eq!(agent_id.as_str(), "agent_reviewer");
    }
}
