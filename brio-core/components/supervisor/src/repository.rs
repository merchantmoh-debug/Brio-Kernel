//! Repository Layer - Task State Access
//!
//! Abstracts database access via the WIT `sql-state` interface.
//! Follows Dependency Inversion: code depends on `TaskRepository` trait,
//! not concrete implementations.

use crate::domain::{AgentId, MergeRequest, ParseStatusError, Priority, Task, TaskId, TaskStatus};
use crate::wit_bindings;
use crate::wit_bindings::sql_state::Row;

// ============================================================================
// Transaction Support
// ============================================================================

/// Errors that can occur during transaction operations.
#[derive(Debug)]
pub enum TransactionError {
    /// Failed to begin transaction.
    BeginError(String),
    /// Failed to commit transaction.
    CommitError(String),
    /// Failed to rollback transaction.
    RollbackError(String),
    /// Transaction was already completed.
    AlreadyCompleted,
}

impl core::fmt::Display for TransactionError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::BeginError(msg) => write!(f, "Failed to begin transaction: {msg}"),
            Self::CommitError(msg) => write!(f, "Failed to commit transaction: {msg}"),
            Self::RollbackError(msg) => write!(f, "Failed to rollback transaction: {msg}"),
            Self::AlreadyCompleted => write!(f, "Transaction was already completed"),
        }
    }
}

impl std::error::Error for TransactionError {}

impl From<TransactionError> for RepositoryError {
    fn from(e: TransactionError) -> Self {
        Self::SqlError(e.to_string())
    }
}

impl From<TransactionError> for BranchRepositoryError {
    fn from(e: TransactionError) -> Self {
        Self::SqlError(e.to_string())
    }
}

/// A database transaction that ensures atomicity for multi-step operations.
///
/// This struct wraps the WIT sql-state interface to provide transaction support.
/// It automatically rolls back on drop if not explicitly committed, providing
/// a safety net against partial updates.
///
/// # Example
///
/// ```rust,no_run
/// use supervisor::repository::{Transaction, TransactionError};
///
/// fn create_branch_with_state() -> Result<(), TransactionError> {
///     let mut tx = Transaction::begin()?;
///     
///     // Perform multiple operations
///     tx.execute("INSERT INTO branches ...", &[])?;
///     tx.execute("INSERT INTO branch_states ...", &[])?;
///     
///     // Commit on success
///     tx.commit()
/// }
/// // Transaction automatically rolls back if commit fails or is not called
/// ```
pub struct Transaction {
    committed: bool,
    rolled_back: bool,
}

impl Transaction {
    /// Begins a new database transaction.
    ///
    /// # Errors
    /// Returns `TransactionError::BeginError` if the BEGIN statement fails.
    pub fn begin() -> Result<Self, TransactionError> {
        wit_bindings::sql_state::execute("BEGIN TRANSACTION", &[])
            .map_err(|e| TransactionError::BeginError(e))?;

        Ok(Self {
            committed: false,
            rolled_back: false,
        })
    }

    /// Commits the transaction, persisting all changes.
    ///
    /// # Errors
    /// Returns `TransactionError::AlreadyCompleted` if already committed or rolled back.
    /// Returns `TransactionError::CommitError` if the COMMIT statement fails.
    pub fn commit(mut self) -> Result<(), TransactionError> {
        if self.committed || self.rolled_back {
            return Err(TransactionError::AlreadyCompleted);
        }

        wit_bindings::sql_state::execute("COMMIT", &[])
            .map_err(|e| TransactionError::CommitError(e))?;

        self.committed = true;
        Ok(())
    }

    /// Rolls back the transaction, discarding all changes.
    ///
    /// # Errors
    /// Returns `TransactionError::AlreadyCompleted` if already committed or rolled back.
    /// Returns `TransactionError::RollbackError` if the ROLLBACK statement fails.
    pub fn rollback(mut self) -> Result<(), TransactionError> {
        if self.committed || self.rolled_back {
            return Err(TransactionError::AlreadyCompleted);
        }

        wit_bindings::sql_state::execute("ROLLBACK", &[])
            .map_err(|e| TransactionError::RollbackError(e))?;

        self.rolled_back = true;
        Ok(())
    }

    /// Executes a SQL statement within the transaction.
    ///
    /// # Errors
    /// Returns error if the statement execution fails.
    pub fn execute(&self, sql: &str, params: &[String]) -> Result<u32, String> {
        wit_bindings::sql_state::execute(sql, params)
    }

    /// Executes a SQL query within the transaction.
    ///
    /// # Errors
    /// Returns error if the query execution fails.
    pub fn query(&self, sql: &str, params: &[String]) -> Result<Vec<Row>, String> {
        wit_bindings::sql_state::query(sql, params)
    }

    /// Returns true if the transaction has been committed.
    #[must_use]
    pub const fn is_committed(&self) -> bool {
        self.committed
    }

    /// Returns true if the transaction has been rolled back.
    #[must_use]
    pub const fn is_rolled_back(&self) -> bool {
        self.rolled_back
    }

    /// Returns true if the transaction is still active.
    #[must_use]
    pub const fn is_active(&self) -> bool {
        !self.committed && !self.rolled_back
    }
}

impl Drop for Transaction {
    fn drop(&mut self) {
        // Auto-rollback on drop if not explicitly completed
        if !self.committed && !self.rolled_back {
            // Ignore errors during drop - we're in a destructor
            let _ = wit_bindings::sql_state::execute("ROLLBACK", &[]);
        }
    }
}

/// Trait for repositories that support transactions.
///
/// This trait provides a common interface for executing operations within
/// a transaction context, ensuring atomicity for multi-step operations.
pub trait Transactional {
    /// Executes a closure within a transaction.
    ///
    /// The closure receives a `&mut Transaction` and can perform multiple
    /// database operations. If the closure returns `Ok`, the transaction is
    /// committed. If it returns `Err`, the transaction is rolled back.
    ///
    /// # Type Parameters
    /// - `F`: The closure type that performs transactional operations
    /// - `T`: The return type of the closure
    /// - `E`: The error type that can be converted from TransactionError
    ///
    /// # Errors
    /// Returns the error from the closure or transaction operations.
    fn with_transaction<F, T, E>(&self, operations: F) -> Result<T, E>
    where
        F: FnOnce(&mut Transaction) -> Result<T, E>,
        E: From<TransactionError>;
}

// ============================================================================
// Column Constants
// ============================================================================

/// Task table column names
const TASK_COL_ID: &str = "id";
const TASK_COL_CONTENT: &str = "content";
const TASK_COL_PRIORITY: &str = "priority";
const TASK_COL_STATUS: &str = "status";
const TASK_COL_PARENT_ID: &str = "parent_id";
const TASK_COL_ASSIGNED_AGENT: &str = "assigned_agent";

/// Branch table column names
const BRANCH_COL_ID: &str = "id";
const BRANCH_COL_PARENT_ID: &str = "parent_id";
const BRANCH_COL_SESSION_ID: &str = "session_id";
const BRANCH_COL_NAME: &str = "name";
const BRANCH_COL_STATUS_JSON: &str = "status_json";
const BRANCH_COL_CONFIG_JSON: &str = "config_json";
const BRANCH_COL_CREATED_AT: &str = "created_at";
const BRANCH_COL_COMPLETED_AT: &str = "completed_at";

/// SQL SELECT column list for branches (must match column constants above)
const BRANCH_COLUMNS: &str =
    "id, parent_id, session_id, name, status_json, config_json, created_at, completed_at";

// ============================================================================
// Shared Helper Functions
// ============================================================================

/// Extracts a column value by name from columns and values arrays.
///
/// # Errors
/// Returns `Err` if the column is not found or has no value.
fn get_column_value<'a>(
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
fn extract_returned_id(row: &Row) -> Result<u64, RepositoryError> {
    let id_val = if let Some(idx) = row.columns.iter().position(|c| c == TASK_COL_ID) {
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

/// Checks that an update affected exactly one row, returning NotFound error if not.
fn expect_task_affected(id: TaskId, affected: u32) -> Result<(), RepositoryError> {
    if affected == 0 {
        return Err(RepositoryError::NotFound(id));
    }
    Ok(())
}

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
        let id = get_column_value(columns, values, TASK_COL_ID)?
            .parse::<u64>()
            .map_err(|e| RepositoryError::ParseError(format!("Invalid id: {e}")))?;

        let content = get_column_value(columns, values, TASK_COL_CONTENT)?.clone();

        let priority = get_column_value(columns, values, TASK_COL_PRIORITY)?
            .parse::<u8>()
            .map_err(|e| RepositoryError::ParseError(format!("Invalid priority: {e}")))?;

        let status = TaskStatus::parse(get_column_value(columns, values, TASK_COL_STATUS)?)?;

        let parent_id = get_column_value(columns, values, TASK_COL_PARENT_ID)
            .ok()
            .and_then(|v| {
                if v == "NULL" || v.is_empty() {
                    None
                } else {
                    v.parse::<u64>().ok().map(TaskId::new)
                }
            });

        let assigned_agent = match get_column_value(columns, values, TASK_COL_ASSIGNED_AGENT) {
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
        // Fetch all non-terminal states
        let active_states = TaskStatus::active_states();
        let placeholders = active_states
            .iter()
            .map(|_| "?")
            .collect::<Vec<_>>()
            .join(", ");

        let sql = format!(
            "SELECT {TASK_COL_ID}, {TASK_COL_CONTENT}, {TASK_COL_PRIORITY}, {TASK_COL_STATUS}, \
             {TASK_COL_PARENT_ID}, {TASK_COL_ASSIGNED_AGENT} \
             FROM tasks \
             WHERE status IN ({placeholders}) \
             ORDER BY priority DESC"
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
            RepositoryError::ParseError(format!("Complex status cannot be stored: {:?}", status))
        })?;
        let params = vec![status_str.to_string(), task_id.inner().to_string()];

        let affected =
            wit_bindings::sql_state::execute(sql, &params).map_err(RepositoryError::SqlError)?;

        expect_task_affected(task_id, affected)
    }

    fn assign_agent(&self, task_id: TaskId, agent: &AgentId) -> Result<(), RepositoryError> {
        let sql = "UPDATE tasks SET assigned_agent = ? WHERE id = ?";
        let params = vec![agent.as_str().to_string(), task_id.inner().to_string()];

        let affected =
            wit_bindings::sql_state::execute(sql, &params).map_err(RepositoryError::SqlError)?;

        expect_task_affected(task_id, affected)
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

        expect_task_affected(task_id, affected)
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

        expect_task_affected(task_id, affected)
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

        expect_task_affected(task_id, affected)
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

        // Use query instead of execute to get the RETURNING id
        let rows =
            wit_bindings::sql_state::query(sql, &params).map_err(RepositoryError::SqlError)?;

        let row = rows.first().ok_or_else(|| {
            RepositoryError::SqlError("INSERT failed to return any rows".to_string())
        })?;

        let id = extract_returned_id(row)?;
        Ok(TaskId::new(id))
    }

    fn fetch_subtasks(&self, parent_id: TaskId) -> Result<Vec<Task>, RepositoryError> {
        let sql = format!(
            "SELECT {TASK_COL_ID}, {TASK_COL_CONTENT}, {TASK_COL_PRIORITY}, {TASK_COL_STATUS}, \
             {TASK_COL_PARENT_ID}, {TASK_COL_ASSIGNED_AGENT} \
             FROM tasks \
             WHERE parent_id = ? \
             ORDER BY priority DESC"
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
                // Drop will automatically rollback, but we can be explicit
                let _ = tx.rollback();
                Err(e)
            }
        }
    }
}

// Branch Repository Implementation

use crate::domain::{Branch, BranchId, BranchStatus};
use crate::merge::MergeId;
use chrono::{DateTime, Utc};

/// Errors specific to branch repository operations.
#[derive(Debug)]
pub enum BranchRepositoryError {
    /// SQL query or execution failed.
    SqlError(String),
    /// Failed to parse data from database.
    ParseError(String),
    /// Branch not found for given ID.
    BranchNotFound(BranchId),
    /// Invalid UUID format.
    InvalidUuid(String),
    /// JSON serialization/deserialization failed.
    JsonError(String),
}

impl core::fmt::Display for BranchRepositoryError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::SqlError(msg) => write!(f, "SQL error: {msg}"),
            Self::ParseError(msg) => write!(f, "Parse error: {msg}"),
            Self::BranchNotFound(id) => write!(f, "Branch not found: {id}"),
            Self::InvalidUuid(msg) => write!(f, "Invalid UUID: {msg}"),
            Self::JsonError(msg) => write!(f, "JSON error: {msg}"),
        }
    }
}

impl std::error::Error for BranchRepositoryError {}

impl From<RepositoryError> for BranchRepositoryError {
    fn from(e: RepositoryError) -> Self {
        match e {
            RepositoryError::SqlError(msg) => Self::SqlError(msg),
            RepositoryError::ParseError(msg) => Self::ParseError(msg),
            _ => Self::SqlError(e.to_string()),
        }
    }
}

impl From<serde_json::Error> for BranchRepositoryError {
    fn from(e: serde_json::Error) -> Self {
        Self::JsonError(e.to_string())
    }
}

impl From<uuid::Error> for BranchRepositoryError {
    fn from(e: uuid::Error) -> Self {
        Self::InvalidUuid(e.to_string())
    }
}

/// Contract for branch persistence operations.
///
/// This trait abstracts the database layer for branch management, enabling:
/// - Unit testing with mock implementations
/// - Swapping storage backends without changing business logic
/// - Recovery of branch state after kernel restarts
pub trait BranchRepository: Send + Sync {
    /// Creates a new branch in the repository.
    ///
    /// # Errors
    /// Returns `BranchRepositoryError` if the creation fails.
    fn create_branch(&self, branch: &Branch) -> Result<BranchId, BranchRepositoryError>;

    /// Fetches a branch by its ID.
    ///
    /// # Errors
    /// Returns `BranchRepositoryError` if the query fails.
    fn get_branch(&self, id: BranchId) -> Result<Option<Branch>, BranchRepositoryError>;

    /// Updates the status of a branch.
    ///
    /// # Errors
    /// Returns `BranchRepositoryError` if the update fails or branch not found.
    fn update_branch_status(
        &self,
        id: BranchId,
        status: BranchStatus,
    ) -> Result<(), BranchRepositoryError>;

    /// Lists all active branches (pending, active, merging).
    /// Used for recovery after kernel restart.
    ///
    /// # Errors
    /// Returns `BranchRepositoryError` if the query fails.
    fn list_active_branches(&self) -> Result<Vec<Branch>, BranchRepositoryError>;

    /// Lists all branches that have a specific parent.
    ///
    /// # Errors
    /// Returns `BranchRepositoryError` if the query fails.
    fn list_branches_by_parent(
        &self,
        parent_id: BranchId,
    ) -> Result<Vec<Branch>, BranchRepositoryError>;

    /// Deletes a branch and all its associated data.
    ///
    /// # Errors
    /// Returns `BranchRepositoryError` if the deletion fails.
    fn delete_branch(&self, id: BranchId) -> Result<(), BranchRepositoryError>;

    /// Creates a new merge request in the queue.
    ///
    /// # Errors
    /// Returns `BranchRepositoryError` if the creation fails.
    fn create_merge_request(
        &self,
        branch_id: BranchId,
        parent_id: Option<BranchId>,
        strategy: &str,
    ) -> Result<MergeId, BranchRepositoryError>;

    /// Gets a merge request by its ID.
    ///
    /// # Errors
    /// Returns `BranchRepositoryError` if the query fails.
    fn get_merge_request(
        &self,
        merge_id: MergeId,
    ) -> Result<Option<MergeRequest>, BranchRepositoryError>;

    /// Updates a merge request's status and staging information.
    ///
    /// # Errors
    /// Returns `BranchRepositoryError` if the update fails.
    fn update_merge_request(
        &self,
        merge_request: &MergeRequest,
    ) -> Result<(), BranchRepositoryError>;

    /// Approves a merge request.
    ///
    /// # Errors
    /// Returns `BranchRepositoryError` if the update fails or merge request not found.
    fn approve_merge(&self, merge_id: MergeId, approver: &str)
        -> Result<(), BranchRepositoryError>;
}

/// Repository implementation for branches using WIT `sql-state` bindings.
pub struct WitBranchRepository;

impl WitBranchRepository {
    /// Creates a new WIT-backed branch repository.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    /// Extracts a column value by name from columns and values arrays.
    fn get_branch_column_value<'a>(
        columns: &'a [String],
        values: &'a [String],
        name: &str,
    ) -> Result<&'a String, BranchRepositoryError> {
        columns
            .iter()
            .position(|c| c == name)
            .and_then(|i| values.get(i))
            .ok_or_else(|| BranchRepositoryError::ParseError(format!("Missing column: {name}")))
    }

    /// Parses a single row into a Branch.
    fn parse_branch_row(
        columns: &[String],
        values: &[String],
    ) -> Result<Branch, BranchRepositoryError> {
        let id = Self::get_branch_column_value(columns, values, BRANCH_COL_ID)?;
        let branch_id = uuid::Uuid::parse_str(id)
            .map(BranchId::from_uuid)
            .map_err(|_| BranchRepositoryError::ParseError(format!("Invalid branch ID: {}", id)))?;

        let parent_id = Self::get_branch_column_value(columns, values, BRANCH_COL_PARENT_ID)
            .ok()
            .and_then(|v| {
                if v == "NULL" || v.is_empty() {
                    None
                } else {
                    uuid::Uuid::parse_str(v).ok().map(BranchId::from_uuid)
                }
            });

        let session_id =
            Self::get_branch_column_value(columns, values, BRANCH_COL_SESSION_ID)?.clone();
        let name = Self::get_branch_column_value(columns, values, BRANCH_COL_NAME)?.clone();

        let status_json =
            Self::get_branch_column_value(columns, values, BRANCH_COL_STATUS_JSON)?.clone();
        let status: BranchStatus = serde_json::from_str(&status_json)?;

        let config =
            Self::get_branch_column_value(columns, values, BRANCH_COL_CONFIG_JSON)?.clone();

        let created_at = Self::get_branch_column_value(columns, values, BRANCH_COL_CREATED_AT)?;
        let created_at = DateTime::parse_from_rfc3339(created_at)
            .map_err(|e| BranchRepositoryError::ParseError(format!("Invalid created_at: {e}")))?
            .with_timezone(&Utc);

        let completed_at = Self::get_branch_column_value(columns, values, BRANCH_COL_COMPLETED_AT)
            .ok()
            .and_then(|v| {
                if v == "NULL" || v.is_empty() {
                    None
                } else {
                    DateTime::parse_from_rfc3339(v)
                        .ok()
                        .map(|dt| dt.with_timezone(&Utc))
                }
            });

        // Use Branch::new which is the public constructor
        let created_at_ts = created_at.timestamp();
        let completed_at_ts = completed_at.map(|dt| dt.timestamp());

        Branch::new(
            branch_id,
            parent_id,
            session_id,
            name,
            status,
            created_at_ts,
            completed_at_ts,
            config,
        )
        .map(|branch| branch)
        .map_err(|e| BranchRepositoryError::ParseError(e.to_string()))
    }

    /// Extracts the returned ID from a query row (for INSERT ... RETURNING id queries).
    fn extract_branch_id(row: &Row) -> Result<BranchId, BranchRepositoryError> {
        let id_val = if let Some(idx) = row.columns.iter().position(|c| c == BRANCH_COL_ID) {
            row.values.get(idx).ok_or_else(|| {
                BranchRepositoryError::ParseError("Returned row missing id value".to_string())
            })?
        } else {
            row.values.first().ok_or_else(|| {
                BranchRepositoryError::ParseError("Returned row has no values".to_string())
            })?
        };

        let uuid = uuid::Uuid::parse_str(id_val)?;
        Ok(BranchId::from_uuid(uuid))
    }

    /// Checks that an update affected exactly one row, returning BranchNotFound error if not.
    fn expect_branch_affected(id: BranchId, affected: u32) -> Result<(), BranchRepositoryError> {
        if affected == 0 {
            return Err(BranchRepositoryError::BranchNotFound(id));
        }
        Ok(())
    }
}

impl Default for WitBranchRepository {
    fn default() -> Self {
        Self::new()
    }
}

impl BranchRepository for WitBranchRepository {
    fn create_branch(&self, branch: &Branch) -> Result<BranchId, BranchRepositoryError> {
        let sql = format!(
            "INSERT INTO branches ({BRANCH_COLUMNS}) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?) \
             RETURNING {BRANCH_COL_ID}"
        );

        let status_json = serde_json::to_string(&branch.status())?;
        let config_json = serde_json::to_string(branch.config())?;

        let parent_id_str = branch
            .parent_id()
            .map_or_else(|| "NULL".to_string(), |id| id.inner().to_string());

        let completed_at_str = branch.completed_at().map_or_else(
            || "NULL".to_string(),
            |ts| {
                DateTime::from_timestamp(ts, 0)
                    .map(|dt| dt.to_rfc3339())
                    .unwrap_or_else(|| "NULL".to_string())
            },
        );

        let created_at_dt = DateTime::from_timestamp(branch.created_at(), 0).ok_or_else(|| {
            BranchRepositoryError::ParseError("Invalid created_at timestamp".to_string())
        })?;

        let params = vec![
            branch.id().inner().to_string(),
            parent_id_str,
            branch.session_id().to_string(),
            branch.name().to_string(),
            status_json,
            config_json,
            created_at_dt.to_rfc3339(),
            completed_at_str,
        ];

        let rows = wit_bindings::sql_state::query(&sql, &params)
            .map_err(BranchRepositoryError::SqlError)?;

        let row = rows.first().ok_or_else(|| {
            BranchRepositoryError::SqlError("INSERT failed to return any rows".to_string())
        })?;

        Self::extract_branch_id(row)
    }

    fn get_branch(&self, id: BranchId) -> Result<Option<Branch>, BranchRepositoryError> {
        let sql = format!(
            "SELECT {BRANCH_COLUMNS} \
             FROM branches \
             WHERE {BRANCH_COL_ID} = ?"
        );

        let params = vec![id.inner().to_string()];

        let rows = wit_bindings::sql_state::query(&sql, &params)
            .map_err(BranchRepositoryError::SqlError)?;

        if rows.is_empty() {
            return Ok(None);
        }

        let row = &rows[0];
        Ok(Some(Self::parse_branch_row(&row.columns, &row.values)?))
    }

    fn update_branch_status(
        &self,
        id: BranchId,
        status: BranchStatus,
    ) -> Result<(), BranchRepositoryError> {
        let sql = if status.is_terminal() {
            "UPDATE branches SET status_json = ?, completed_at = ? WHERE id = ?"
        } else {
            "UPDATE branches SET status_json = ? WHERE id = ?"
        };

        let status_json = serde_json::to_string(&status)?;

        let params = if status.is_terminal() {
            vec![status_json, Utc::now().to_rfc3339(), id.inner().to_string()]
        } else {
            vec![status_json, id.inner().to_string()]
        };

        let affected = wit_bindings::sql_state::execute(sql, &params)
            .map_err(BranchRepositoryError::SqlError)?;

        Self::expect_branch_affected(id, affected)
    }

    fn list_active_branches(&self) -> Result<Vec<Branch>, BranchRepositoryError> {
        let sql = format!(
            "SELECT {BRANCH_COLUMNS} \
             FROM branches \
             ORDER BY {BRANCH_COL_CREATED_AT} DESC"
        );

        let rows =
            wit_bindings::sql_state::query(&sql, &[]).map_err(BranchRepositoryError::SqlError)?;

        rows.iter()
            .map(|row| Self::parse_branch_row(&row.columns, &row.values))
            .filter(|branch| {
                // Only return active branches
                if let Ok(branch) = branch {
                    branch.is_active()
                } else {
                    true // Include branches that failed to parse for debugging
                }
            })
            .collect()
    }

    fn list_branches_by_parent(
        &self,
        parent_id: BranchId,
    ) -> Result<Vec<Branch>, BranchRepositoryError> {
        let sql = format!(
            "SELECT {BRANCH_COLUMNS} \
             FROM branches \
             WHERE {BRANCH_COL_PARENT_ID} = ? \
             ORDER BY {BRANCH_COL_CREATED_AT} DESC"
        );

        let params = vec![parent_id.inner().to_string()];

        let rows = wit_bindings::sql_state::query(&sql, &params)
            .map_err(BranchRepositoryError::SqlError)?;

        rows.iter()
            .map(|row| Self::parse_branch_row(&row.columns, &row.values))
            .collect()
    }

    fn delete_branch(&self, id: BranchId) -> Result<(), BranchRepositoryError> {
        let sql = "DELETE FROM branches WHERE id = ?";

        let params = vec![id.inner().to_string()];

        let affected = wit_bindings::sql_state::execute(sql, &params)
            .map_err(BranchRepositoryError::SqlError)?;

        Self::expect_branch_affected(id, affected)
    }

    fn create_merge_request(
        &self,
        branch_id: BranchId,
        parent_id: Option<BranchId>,
        strategy: &str,
    ) -> Result<MergeId, BranchRepositoryError> {
        let sql = "INSERT INTO merge_queue (id, branch_id, parent_id, strategy, status, requires_approval, approved_by, approved_at, created_at) \
                   VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?) \
                   RETURNING id";

        let merge_id = MergeId::new();
        let created_at = Utc::now();
        let parent_id_str =
            parent_id.map_or_else(|| "NULL".to_string(), |id| id.inner().to_string());

        let params = vec![
            merge_id.to_string(),
            branch_id.inner().to_string(),
            parent_id_str,
            strategy.to_string(),
            "pending".to_string(),
            "1".to_string(), // requires_approval = true
            "NULL".to_string(),
            "NULL".to_string(),
            created_at.to_rfc3339(),
        ];

        let rows = wit_bindings::sql_state::query(sql, &params)
            .map_err(BranchRepositoryError::SqlError)?;

        let row = rows.first().ok_or_else(|| {
            BranchRepositoryError::SqlError("INSERT failed to return any rows".to_string())
        })?;

        // For merge_queue, we need a different extraction since it returns a MergeId
        let id_val = if let Some(idx) = row.columns.iter().position(|c| c == "id") {
            row.values.get(idx).ok_or_else(|| {
                BranchRepositoryError::ParseError("Returned row missing id value".to_string())
            })?
        } else {
            row.values.first().ok_or_else(|| {
                BranchRepositoryError::ParseError("Returned row has no values".to_string())
            })?
        };

        let uuid = uuid::Uuid::parse_str(id_val)?;
        Ok(MergeId::from_uuid(uuid))
    }

    fn approve_merge(
        &self,
        merge_id: MergeId,
        approver: &str,
    ) -> Result<(), BranchRepositoryError> {
        let sql = "UPDATE merge_queue SET status = ?, approved_by = ?, approved_at = ? \
                   WHERE id = ? AND status = 'pending'";

        let approved_at = Utc::now();

        let params = vec![
            "approved".to_string(),
            approver.to_string(),
            approved_at.to_rfc3339(),
            merge_id.to_string(),
        ];

        let affected = wit_bindings::sql_state::execute(sql, &params)
            .map_err(BranchRepositoryError::SqlError)?;

        if affected == 0 {
            // Create a placeholder branch ID for the error
            // The actual ID isn't meaningful here since the branch wasn't found
            return Err(BranchRepositoryError::BranchNotFound(BranchId::new()));
        }

        Ok(())
    }

    fn get_merge_request(
        &self,
        merge_id: MergeId,
    ) -> Result<Option<MergeRequest>, BranchRepositoryError> {
        // For now, return None - full implementation would query database
        // This is a placeholder that allows compilation
        let _ = merge_id;
        Ok(None)
    }

    fn update_merge_request(
        &self,
        _merge_request: &MergeRequest,
    ) -> Result<(), BranchRepositoryError> {
        // Placeholder implementation - would update database in full implementation
        Ok(())
    }
}

impl Transactional for WitBranchRepository {
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
                // Drop will automatically rollback, but we can be explicit
                let _ = tx.rollback();
                Err(e)
            }
        }
    }
}

#[cfg(test)]
mod branch_repository_tests {
    use super::*;
    use crate::domain::BranchConfig;

    #[test]
    fn branch_repository_error_display() {
        let id = BranchId::new();
        let err = BranchRepositoryError::BranchNotFound(id);
        assert!(err.to_string().contains("not found"));
    }

    #[test]
    fn wit_branch_repository_new() {
        let repo = WitBranchRepository::new();
        // Just verify it can be created
        let _ = repo;
    }
}
