//! Transaction Support
//!
//! Provides database transaction management with automatic rollback on drop.

use crate::wit_bindings;
use crate::wit_bindings::sql_state::Row;

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
    /// SQL execution error.
    SqlError(String),
}

impl core::fmt::Display for TransactionError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::BeginError(msg) => write!(f, "Failed to begin transaction: {msg}"),
            Self::CommitError(msg) => write!(f, "Failed to commit transaction: {msg}"),
            Self::RollbackError(msg) => write!(f, "Failed to rollback transaction: {msg}"),
            Self::AlreadyCompleted => write!(f, "Transaction was already completed"),
            Self::SqlError(msg) => write!(f, "SQL error: {msg}"),
        }
    }
}

impl std::error::Error for TransactionError {}

impl From<String> for TransactionError {
    fn from(s: String) -> Self {
        Self::SqlError(s)
    }
}

impl From<TransactionError> for super::RepositoryError {
    fn from(e: TransactionError) -> Self {
        Self::SqlError(e.to_string())
    }
}

impl From<TransactionError> for super::BranchRepositoryError {
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
            .map_err(TransactionError::BeginError)?;

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

        wit_bindings::sql_state::execute("COMMIT", &[]).map_err(TransactionError::CommitError)?;

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
            .map_err(TransactionError::RollbackError)?;

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
    /// The closure receives a `Transaction` and can perform multiple
    /// database operations. If the closure returns `Ok`, the transaction is
    /// committed. If it returns `Err`, the transaction is rolled back.
    ///
    /// # Type Parameters
    /// - `F`: The closure type that performs transactional operations
    /// - `T`: The return type of the closure
    /// - `E`: The error type that can be converted from `TransactionError`
    ///
    /// # Errors
    /// Returns the error from the closure or transaction operations.
    fn with_transaction<F, T, E>(&self, operations: F) -> Result<T, E>
    where
        F: FnOnce(&mut Transaction) -> Result<T, E>,
        E: From<TransactionError>;
}
