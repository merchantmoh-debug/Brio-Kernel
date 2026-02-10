//! Repository Layer - Task State Access
//!
//! Abstracts database access via the WIT `sql-state` interface.
//! Follows Dependency Inversion: code depends on `TaskRepository` trait,
//! not concrete implementations.

// Re-export all public items from submodules
pub use branch::{BranchRepository, BranchRepositoryError, WitBranchRepository};
pub use column::{BRANCH_COLS, RepositoryError, TASK_COLS, get_column_value};
pub use task::{TaskRepository, WitTaskRepository};
pub use transaction::{Transaction, TransactionError, Transactional};

// Declare submodules
mod branch;
mod column;
mod task;
mod transaction;
