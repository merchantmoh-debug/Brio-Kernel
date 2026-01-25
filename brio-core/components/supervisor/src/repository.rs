//! Repository Layer - Task State Access
//!
//! Abstracts database access via the WIT `sql-state` interface.
//! Follows Dependency Inversion: code depends on `TaskRepository` trait,
//! not concrete implementations.

use crate::domain::{AgentId, ParseStatusError, Priority, Task, TaskId, TaskStatus};
use crate::wit_bindings;

// =============================================================================
// Error Types
// =============================================================================

/// Errors that can occur during repository operations.
#[derive(Debug)]
pub enum RepositoryError {
    /// SQL query or execution failed.
    SqlError(String),
    /// Failed to parse data from database.
    ParseError(String),
    /// Task not found for given ID.
    NotFound(TaskId),
}

impl core::fmt::Display for RepositoryError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::SqlError(msg) => write!(f, "SQL error: {msg}"),
            Self::ParseError(msg) => write!(f, "Parse error: {msg}"),
            Self::NotFound(id) => write!(f, "Task not found: {id}"),
        }
    }
}

impl std::error::Error for RepositoryError {}

impl From<ParseStatusError> for RepositoryError {
    fn from(e: ParseStatusError) -> Self {
        Self::ParseError(e.to_string())
    }
}

// =============================================================================
// Repository Trait (Dependency Inversion)
// =============================================================================

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
}

// =============================================================================
// WIT Implementation
// =============================================================================

/// Repository implementation using WIT `sql-state` bindings.
pub struct WitTaskRepository;

impl WitTaskRepository {
    /// Creates a new WIT-backed repository.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    /// Parses a single row into a Task.
    fn parse_row(columns: &[String], values: &[String]) -> Result<Task, RepositoryError> {
        let get_value = |name: &str| -> Result<&String, RepositoryError> {
            columns
                .iter()
                .position(|c| c == name)
                .and_then(|i| values.get(i))
                .ok_or_else(|| RepositoryError::ParseError(format!("Missing column: {name}")))
        };

        let id = get_value("id")?
            .parse::<u64>()
            .map_err(|e| RepositoryError::ParseError(format!("Invalid id: {e}")))?;

        let content = get_value("content")?.clone();

        let priority = get_value("priority")?
            .parse::<u8>()
            .map_err(|e| RepositoryError::ParseError(format!("Invalid priority: {e}")))?;

        let status = TaskStatus::parse(get_value("status")?)?;

        let assigned_agent = get_value("assigned_agent").ok().and_then(|v| {
            if v == "NULL" || v.is_empty() {
                None
            } else {
                Some(AgentId::new(v.clone()))
            }
        });

        Ok(Task::new(
            TaskId::new(id),
            content,
            Priority::new(priority),
            status,
            assigned_agent,
        ))
    }
}

impl Default for WitTaskRepository {
    fn default() -> Self {
        Self::new()
    }
}

impl TaskRepository for WitTaskRepository {
    fn fetch_active_tasks(&self) -> Result<Vec<Task>, RepositoryError> {
        // Fetch all non-terminal states
        let sql = "SELECT id, content, priority, status, assigned_agent \
                   FROM tasks \
                   WHERE status IN (?, ?, ?, ?) \
                   ORDER BY priority DESC";

        let params = vec![
            TaskStatus::Pending.as_str().to_string(),
            TaskStatus::Planning.as_str().to_string(),
            TaskStatus::Executing.as_str().to_string(),
            TaskStatus::Verifying.as_str().to_string(),
        ];

        let rows =
            wit_bindings::sql_state::query(sql, &params).map_err(RepositoryError::SqlError)?;

        rows.iter()
            .map(|row| Self::parse_row(&row.columns, &row.values))
            .collect()
    }

    fn update_status(&self, task_id: TaskId, status: TaskStatus) -> Result<(), RepositoryError> {
        let sql = "UPDATE tasks SET status = ? WHERE id = ?";
        let params = vec![status.as_str().to_string(), task_id.inner().to_string()];

        let affected =
            wit_bindings::sql_state::execute(sql, &params).map_err(RepositoryError::SqlError)?;

        if affected == 0 {
            return Err(RepositoryError::NotFound(task_id));
        }
        Ok(())
    }

    fn assign_agent(&self, task_id: TaskId, agent: &AgentId) -> Result<(), RepositoryError> {
        let sql = "UPDATE tasks SET assigned_agent = ? WHERE id = ?";
        let params = vec![agent.as_str().to_string(), task_id.inner().to_string()];

        let affected =
            wit_bindings::sql_state::execute(sql, &params).map_err(RepositoryError::SqlError)?;

        if affected == 0 {
            return Err(RepositoryError::NotFound(task_id));
        }
        Ok(())
    }

    fn mark_assigned(&self, task_id: TaskId, agent: &AgentId) -> Result<(), RepositoryError> {
        let sql = "UPDATE tasks SET status = ?, assigned_agent = ? WHERE id = ?";

        let params = vec![
            TaskStatus::Assigned.as_str().to_string(),
            agent.as_str().to_string(),
            task_id.inner().to_string(),
        ];

        let affected =
            wit_bindings::sql_state::execute(sql, &params).map_err(RepositoryError::SqlError)?;

        if affected == 0 {
            return Err(RepositoryError::NotFound(task_id));
        }

        Ok(())
    }

    fn mark_completed(&self, task_id: TaskId) -> Result<(), RepositoryError> {
        let sql = "UPDATE tasks SET status = ? WHERE id = ?";

        let params = vec![
            TaskStatus::Completed.as_str().to_string(),
            task_id.inner().to_string(),
        ];

        let affected =
            wit_bindings::sql_state::execute(sql, &params).map_err(RepositoryError::SqlError)?;

        if affected == 0 {
            return Err(RepositoryError::NotFound(task_id));
        }

        Ok(())
    }

    fn mark_failed(&self, task_id: TaskId, reason: &str) -> Result<(), RepositoryError> {
        let sql = "UPDATE tasks SET status = ?, failure_reason = ? WHERE id = ?";

        let params = vec![
            TaskStatus::Failed.as_str().to_string(),
            reason.to_string(),
            task_id.inner().to_string(),
        ];

        let affected =
            wit_bindings::sql_state::execute(sql, &params).map_err(RepositoryError::SqlError)?;

        if affected == 0 {
            return Err(RepositoryError::NotFound(task_id));
        }

        Ok(())
    }
}
