//! Task Repository - Traits
//!
//! This module defines the `TaskRepository` trait.

use crate::domain::{AgentId, Priority, Task, TaskId, TaskStatus};
use crate::repository::column::RepositoryError;

/// Contract for task state access.
///
/// This trait abstracts the database layer, enabling:
/// - Unit testing with mock implementations
/// - Swapping storage backends without changing business logic
pub trait TaskRepository {
    /// Fetches all active tasks (not completed or failed), ordered by priority DESC.
    ///
    /// # Errors
    /// Returns `RepositoryError` if the query fails or data is malformed.
    fn fetch_active_tasks(&self) -> Result<Vec<Task>, RepositoryError>;

    /// Updates task status.
    ///
    /// # Errors
    /// Returns `RepositoryError` if the update fails.
    fn update_status(&self, task_id: TaskId, status: TaskStatus) -> Result<(), RepositoryError>;

    /// Assigns an agent to a task without changing its status.
    ///
    /// # Errors
    /// Returns `RepositoryError` if the update fails.
    fn assign_agent(&self, task_id: TaskId, agent: &AgentId) -> Result<(), RepositoryError>;

    /// Marks a task as assigned to an agent (Legacy: sets status to Assigned).
    ///
    /// # Errors
    /// Returns `RepositoryError` if the update fails.
    fn mark_assigned(&self, task_id: TaskId, agent: &AgentId) -> Result<(), RepositoryError>;

    /// Marks a task as completed.
    ///
    /// # Errors
    /// Returns `RepositoryError` if the update fails.
    fn mark_completed(&self, task_id: TaskId) -> Result<(), RepositoryError>;

    /// Marks a task as failed with a reason.
    ///
    /// # Errors
    /// Returns `RepositoryError` if the update fails.
    fn mark_failed(&self, task_id: TaskId, reason: &str) -> Result<(), RepositoryError>;

    /// Creates a new task in the repository.
    ///
    /// # Errors
    /// Returns `RepositoryError` if the creation fails.
    fn create_task(
        &self,
        content: String,
        priority: Priority,
        parent_id: Option<TaskId>,
    ) -> Result<TaskId, RepositoryError>;

    /// Fetches all subtasks for a given parent task.
    ///
    /// # Errors
    /// Returns `RepositoryError` if the query fails.
    fn fetch_subtasks(&self, parent_id: TaskId) -> Result<Vec<Task>, RepositoryError>;
}
