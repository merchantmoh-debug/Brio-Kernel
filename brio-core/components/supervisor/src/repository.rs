//! Repository Layer - Task State Access
//!
//! Abstracts database access via the WIT `sql-state` interface.
//! Follows Dependency Inversion: code depends on `TaskRepository` trait,
//! not concrete implementations.

use crate::domain::{AgentId, ParseStatusError, Priority, Task, TaskId, TaskStatus};
use crate::wit_bindings;

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
        let get_column_value = |name: &str| -> Result<&String, RepositoryError> {
            columns
                .iter()
                .position(|c| c == name)
                .and_then(|i| values.get(i))
                .ok_or_else(|| RepositoryError::ParseError(format!("Missing column: {name}")))
        };

        let id = get_column_value("id")?
            .parse::<u64>()
            .map_err(|e| RepositoryError::ParseError(format!("Invalid id: {e}")))?;

        let content = get_column_value("content")?.clone();

        let priority = get_column_value("priority")?
            .parse::<u8>()
            .map_err(|e| RepositoryError::ParseError(format!("Invalid priority: {e}")))?;

        let status = TaskStatus::parse(get_column_value("status")?)?;

        let parent_id = get_column_value("parent_id").ok().and_then(|v| {
            if v == "NULL" || v.is_empty() {
                None
            } else {
                v.parse::<u64>().ok().map(TaskId::new)
            }
        });

        let assigned_agent = get_column_value("assigned_agent").ok().and_then(|v| {
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
            parent_id,
            assigned_agent,
            std::collections::HashSet::new(),
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
        let active_states = TaskStatus::active_states();
        let placeholders = active_states
            .iter()
            .map(|_| "?")
            .collect::<Vec<_>>()
            .join(", ");

        let sql = format!(
            "SELECT id, content, priority, status, parent_id, assigned_agent \
             FROM tasks \
             WHERE status IN ({placeholders}) \
             ORDER BY priority DESC"
        );

        let params: Vec<String> = active_states
            .iter()
            .map(|s| s.as_str().to_string())
            .collect();

        let rows =
            wit_bindings::sql_state::query(&sql, &params).map_err(RepositoryError::SqlError)?;

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

    fn create_task(
        &self,
        content: String,
        priority: Priority,
        parent_id: Option<TaskId>,
    ) -> Result<TaskId, RepositoryError> {
        let sql = "INSERT INTO tasks (content, priority, status, parent_id) \
                   VALUES (?, ?, ?, ?) \
                   RETURNING id";

        let parent_id_str =
            parent_id.map_or_else(|| "NULL".to_string(), |id| id.inner().to_string());

        let params = vec![
            content,
            priority.inner().to_string(),
            TaskStatus::Pending.as_str().to_string(),
            parent_id_str,
        ];

        // Use query instead of execute to get the RETURNING id
        let rows =
            wit_bindings::sql_state::query(sql, &params).map_err(RepositoryError::SqlError)?;

        let row = rows.first().ok_or_else(|| {
            RepositoryError::SqlError("INSERT failed to return any rows".to_string())
        })?;

        // Assuming Column "id" is returned.
        // We find the index of "id" or just take the first value if it's the only one returned which is robust.
        let id_val = if let Some(idx) = row.columns.iter().position(|c| c == "id") {
            row.values.get(idx).ok_or_else(|| {
                RepositoryError::ParseError("Returned row missing id value".to_string())
            })?
        } else {
            // Fallback: take first column
            row.values.first().ok_or_else(|| {
                RepositoryError::ParseError("Returned row has no values".to_string())
            })?
        };

        let id = id_val
            .parse::<u64>()
            .map_err(|e| RepositoryError::ParseError(format!("Invalid id returned: {e}")))?;

        Ok(TaskId::new(id))
    }

    fn fetch_subtasks(&self, parent_id: TaskId) -> Result<Vec<Task>, RepositoryError> {
        let sql = "SELECT id, content, priority, status, parent_id, assigned_agent \
                   FROM tasks \
                   WHERE parent_id = ? \
                   ORDER BY priority DESC";

        let params = vec![parent_id.inner().to_string()];

        let rows =
            wit_bindings::sql_state::query(sql, &params).map_err(RepositoryError::SqlError)?;

        rows.iter()
            .map(|row| Self::parse_row(&row.columns, &row.values))
            .collect()
    }
}
