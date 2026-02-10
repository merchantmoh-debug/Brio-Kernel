//! Column Constants and Extraction Helpers
//!
//! Provides shared column names and helper functions for extracting
//! values from database query results.

use crate::wit_bindings::sql_state::Row;

/// Column constants for task table
pub mod task_cols {
    /// Task ID column name
    pub const ID: &str = "id";
    /// Task content column name
    pub const CONTENT: &str = "content";
    /// Task priority column name
    pub const PRIORITY: &str = "priority";
    /// Task status column name
    pub const STATUS: &str = "status";
    /// Task `parent_id` column name
    pub const PARENT_ID: &str = "parent_id";
    /// Task `assigned_agent` column name
    pub const ASSIGNED_AGENT: &str = "assigned_agent";
}

/// Column constants for branch table
pub mod branch_cols {
    /// Branch ID column name
    pub const ID: &str = "id";
    /// Branch `parent_id` column name
    pub const PARENT_ID: &str = "parent_id";
    /// Branch `session_id` column name
    pub const SESSION_ID: &str = "session_id";
    /// Branch name column name
    pub const NAME: &str = "name";
    /// Branch `status_json` column name
    pub const STATUS_JSON: &str = "status_json";
    /// Branch `config_json` column name
    pub const CONFIG_JSON: &str = "config_json";
    /// Branch `created_at` column name
    pub const CREATED_AT: &str = "created_at";
    /// Branch `completed_at` column name
    pub const COMPLETED_AT: &str = "completed_at";
}

/// SQL SELECT column list for branches
pub const BRANCH_COLUMNS: &str =
    "id, parent_id, session_id, name, status_json, config_json, created_at, completed_at";

/// Re-export column modules
pub use branch_cols as BRANCH_COLS;
pub use task_cols as TASK_COLS;

/// Errors that can occur during repository operations.
#[derive(Debug)]
#[allow(dead_code)]
pub enum RepositoryError {
    /// SQL query or execution failed.
    SqlError(String),
    /// Failed to parse data from database.
    ParseError(String),
    /// Task not found for given ID.
    NotFound(crate::domain::TaskId),
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

impl From<crate::domain::ParseStatusError> for RepositoryError {
    fn from(e: crate::domain::ParseStatusError) -> Self {
        Self::ParseError(e.to_string())
    }
}

/// Extracts a column value by name from columns and values arrays.
///
/// # Errors
/// Returns `Err` if the column is not found or has no value.
pub fn get_column_value<'a>(
    columns: &'a [String],
    values: &'a [String],
    name: &str,
) -> Result<&'a String, RepositoryError> {
    columns
        .iter()
        .position(|c| c == name)
        .and_then(|i| values.get(i))
        .ok_or_else(|| RepositoryError::ParseError(format!("Missing column: {name}")))
}

/// Extracts the returned ID from a query row (for INSERT ... RETURNING id queries).
///
/// # Errors
/// Returns `Err` if the row has no values or the ID cannot be parsed.
pub fn extract_returned_id(row: &Row, col_name: &str) -> Result<u64, RepositoryError> {
    let id_val = if let Some(idx) = row.columns.iter().position(|c| c == col_name) {
        row.values.get(idx).ok_or_else(|| {
            RepositoryError::ParseError("Returned row missing id value".to_string())
        })?
    } else {
        row.values
            .first()
            .ok_or_else(|| RepositoryError::ParseError("Returned row has no values".to_string()))?
    };

    id_val
        .parse::<u64>()
        .map_err(|e| RepositoryError::ParseError(format!("Invalid id returned: {e}")))
}

/// Checks that an update affected exactly one row, returning `NotFound` error if not.
pub fn expect_affected(id: impl core::fmt::Display, affected: u32) -> Result<(), RepositoryError> {
    if affected == 0 {
        return Err(RepositoryError::SqlError(format!("Not found: {id}")));
    }
    Ok(())
}
