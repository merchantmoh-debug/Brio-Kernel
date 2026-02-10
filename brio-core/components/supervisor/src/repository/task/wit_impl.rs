//! Task Repository - WIT Implementation
//!
//! This module provides the `WitTaskRepository` implementation.

use crate::domain::{AgentId, Priority, Task, TaskId, TaskStatus};
use crate::repository::column::{
    RepositoryError, expect_affected, extract_returned_id, get_column_value, task_cols,
};
use crate::repository::task::traits::TaskRepository;
use crate::repository::transaction::{Transaction, TransactionError, Transactional};
use crate::wit_bindings;

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
        let id = get_column_value(columns, values, task_cols::ID)?
            .parse::<u64>()
            .map_err(|e| RepositoryError::ParseError(format!("Invalid id: {e}")))?;

        let content = get_column_value(columns, values, task_cols::CONTENT)?.clone();

        let priority = get_column_value(columns, values, task_cols::PRIORITY)?
            .parse::<u8>()
            .map_err(|e| RepositoryError::ParseError(format!("Invalid priority: {e}")))?;

        let status = TaskStatus::parse(get_column_value(columns, values, task_cols::STATUS)?)?;

        let parent_id = get_column_value(columns, values, task_cols::PARENT_ID)
            .ok()
            .and_then(|v| {
                if v == "NULL" || v.is_empty() {
                    None
                } else {
                    v.parse::<u64>().ok().map(TaskId::new)
                }
            });

        let assigned_agent = match get_column_value(columns, values, task_cols::ASSIGNED_AGENT) {
            Ok(v) if v != "NULL" && !v.is_empty() => Some(
                AgentId::new(v.clone()).map_err(|e| RepositoryError::ParseError(e.to_string()))?,
            ),
            _ => None,
        };

        Task::new(
            TaskId::new(id),
            content,
            Priority::new(priority),
            status,
            parent_id,
            assigned_agent,
            std::collections::HashSet::new(),
        )
        .map_err(|e| RepositoryError::ParseError(e.to_string()))
    }
}

impl Default for WitTaskRepository {
    fn default() -> Self {
        Self::new()
    }
}

impl TaskRepository for WitTaskRepository {
    fn fetch_active_tasks(&self) -> Result<Vec<Task>, RepositoryError> {
        let active_states = TaskStatus::active_states();
        let placeholders = active_states
            .iter()
            .map(|_| "?")
            .collect::<Vec<_>>()
            .join(", ");

        let sql = format!(
            "SELECT {}, {}, {}, {}, {}, {} \
             FROM tasks \
             WHERE status IN ({placeholders}) \
             ORDER BY priority DESC",
            task_cols::ID,
            task_cols::CONTENT,
            task_cols::PRIORITY,
            task_cols::STATUS,
            task_cols::PARENT_ID,
            task_cols::ASSIGNED_AGENT
        );

        let params: Vec<String> = active_states
            .iter()
            .filter_map(|s: &TaskStatus| s.as_str().map(|str_val: &str| str_val.to_string()))
            .collect();

        let rows =
            wit_bindings::sql_state::query(&sql, &params).map_err(RepositoryError::SqlError)?;

        rows.iter()
            .map(|row| Self::parse_row(&row.columns, &row.values))
            .collect()
    }

    fn update_status(&self, task_id: TaskId, status: TaskStatus) -> Result<(), RepositoryError> {
        let sql = "UPDATE tasks SET status = ? WHERE id = ?";
        let status_str = status.as_str().ok_or_else(|| {
            RepositoryError::ParseError(format!("Complex status cannot be stored: {status:?}"))
        })?;
        let params = vec![status_str.to_string(), task_id.inner().to_string()];

        let affected =
            wit_bindings::sql_state::execute(sql, &params).map_err(RepositoryError::SqlError)?;

        expect_affected(task_id, affected)
    }

    fn assign_agent(&self, task_id: TaskId, agent: &AgentId) -> Result<(), RepositoryError> {
        let sql = "UPDATE tasks SET assigned_agent = ? WHERE id = ?";
        let params = vec![agent.as_str().to_string(), task_id.inner().to_string()];

        let affected =
            wit_bindings::sql_state::execute(sql, &params).map_err(RepositoryError::SqlError)?;

        expect_affected(task_id, affected)
    }

    fn mark_assigned(&self, task_id: TaskId, agent: &AgentId) -> Result<(), RepositoryError> {
        let sql = "UPDATE tasks SET status = ?, assigned_agent = ? WHERE id = ?";

        let params = vec![
            TaskStatus::Assigned
                .as_str()
                .expect("Assigned is a simple variant")
                .to_string(),
            agent.as_str().to_string(),
            task_id.inner().to_string(),
        ];

        let affected =
            wit_bindings::sql_state::execute(sql, &params).map_err(RepositoryError::SqlError)?;

        expect_affected(task_id, affected)
    }

    fn mark_completed(&self, task_id: TaskId) -> Result<(), RepositoryError> {
        let sql = "UPDATE tasks SET status = ? WHERE id = ?";

        let params = vec![
            TaskStatus::Completed
                .as_str()
                .expect("Completed is a simple variant")
                .to_string(),
            task_id.inner().to_string(),
        ];

        let affected =
            wit_bindings::sql_state::execute(sql, &params).map_err(RepositoryError::SqlError)?;

        expect_affected(task_id, affected)
    }

    fn mark_failed(&self, task_id: TaskId, reason: &str) -> Result<(), RepositoryError> {
        let sql = "UPDATE tasks SET status = ?, failure_reason = ? WHERE id = ?";

        let params = vec![
            TaskStatus::Failed
                .as_str()
                .expect("Failed is a simple variant")
                .to_string(),
            reason.to_string(),
            task_id.inner().to_string(),
        ];

        let affected =
            wit_bindings::sql_state::execute(sql, &params).map_err(RepositoryError::SqlError)?;

        expect_affected(task_id, affected)
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
            TaskStatus::Pending
                .as_str()
                .expect("Pending is a simple variant")
                .to_string(),
            parent_id_str,
        ];

        let rows =
            wit_bindings::sql_state::query(sql, &params).map_err(RepositoryError::SqlError)?;

        let row = rows.first().ok_or_else(|| {
            RepositoryError::SqlError("INSERT failed to return any rows".to_string())
        })?;

        let id = extract_returned_id(row, task_cols::ID)?;
        Ok(TaskId::new(id))
    }

    fn fetch_subtasks(&self, parent_id: TaskId) -> Result<Vec<Task>, RepositoryError> {
        let sql = format!(
            "SELECT {}, {}, {}, {}, {}, {} \
             FROM tasks \
             WHERE parent_id = ? \
             ORDER BY priority DESC",
            task_cols::ID,
            task_cols::CONTENT,
            task_cols::PRIORITY,
            task_cols::STATUS,
            task_cols::PARENT_ID,
            task_cols::ASSIGNED_AGENT
        );

        let params = vec![parent_id.inner().to_string()];

        let rows =
            wit_bindings::sql_state::query(&sql, &params).map_err(RepositoryError::SqlError)?;

        rows.iter()
            .map(|row| Self::parse_row(&row.columns, &row.values))
            .collect()
    }
}

impl Transactional for WitTaskRepository {
    fn with_transaction<F, T, E>(&self, operations: F) -> Result<T, E>
    where
        F: FnOnce(&mut Transaction) -> Result<T, E>,
        E: From<TransactionError>,
    {
        let mut tx = Transaction::begin()?;

        match operations(&mut tx) {
            Ok(result) => {
                tx.commit()?;
                Ok(result)
            }
            Err(e) => {
                let _ = tx.rollback();
                Err(e)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wit_task_repository_new() {
        let repo = WitTaskRepository::new();
        let _ = repo; // Verify it can be created
    }
}
