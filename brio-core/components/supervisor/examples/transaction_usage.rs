//! Transaction API Usage Examples
//!
//! This file demonstrates how to use the transaction support added to the
//! repository layer for atomic multi-step operations.
//!
//! # Overview
//!
//! The transaction system provides:
//! - Manual transaction control via `Transaction::begin()`, `commit()`, `rollback()`
//! - Automatic transaction management via `with_transaction()` closure API
//! - Auto-rollback on drop for safety
//! - Integration with both TaskRepository and BranchRepository

use supervisor::repository::{
    Transaction, TransactionError, Transactional, WitBranchRepository, WitTaskRepository,
};

// =============================================================================
// Example 1: Manual Transaction Control
// =============================================================================

/// Creates a task with subtasks atomically.
///
/// This example shows manual transaction control where both the parent task
/// and subtasks are created in a single atomic operation.
pub fn create_task_with_subtasks_manual(
    content: String,
    subtask_contents: Vec<String>,
) -> Result<u64, TransactionError> {
    // Begin a new transaction
    let mut tx = Transaction::begin()?;

    // Insert the parent task
    let parent_result = tx.query(
        "INSERT INTO tasks (content, priority, status, parent_id) VALUES (?, ?, ?, ?) RETURNING id",
        &vec![
            content,
            "5".to_string(),
            "Pending".to_string(),
            "NULL".to_string(),
        ],
    )?;

    let parent_id = parent_result
        .first()
        .and_then(|row| row.values.first())
        .ok_or_else(|| TransactionError::BeginError("Failed to get parent ID".to_string()))?
        .parse::<u64>()
        .map_err(|e| TransactionError::BeginError(format!("Invalid parent ID: {e}")))?;

    // Insert all subtasks
    for subtask_content in subtask_contents {
        tx.execute(
            "INSERT INTO tasks (content, priority, status, parent_id) VALUES (?, ?, ?, ?)",
            &vec![
                subtask_content,
                "3".to_string(),
                "Pending".to_string(),
                parent_id.to_string(),
            ],
        )?;
    }

    // Commit the transaction - all operations succeed or none do
    tx.commit()?;

    Ok(parent_id)
}

// =============================================================================
// Example 2: Using with_transaction Closure API
// =============================================================================

/// Creates a branch with initial state atomically using the closure API.
///
/// This example demonstrates the `with_transaction` method which automatically
/// handles commit/rollback based on the closure result.
pub fn create_branch_with_state_atomic(
    repo: &WitBranchRepository,
    branch_id: &str,
    session_id: &str,
    name: &str,
    initial_state_json: &str,
) -> Result<(), TransactionError> {
    repo.with_transaction(|tx| {
        // Insert the branch
        tx.execute(
            "INSERT INTO branches (id, session_id, name, status_json, created_at) \
             VALUES (?, ?, ?, ?, datetime('now'))",
            &vec![
                branch_id.to_string(),
                session_id.to_string(),
                name.to_string(),
                initial_state_json.to_string(),
            ],
        )?;

        // Insert initial state record
        tx.execute(
            "INSERT INTO branch_states (branch_id, state_json, created_at) \
             VALUES (?, ?, datetime('now'))",
            &vec![branch_id.to_string(), initial_state_json.to_string()],
        )?;

        Ok(())
    })
}

// =============================================================================
// Example 3: Multi-Table Operations with Error Handling
// =============================================================================

/// Updates task status and logs the change atomically.
///
/// This example shows how to handle errors within a transaction and
/// how the transaction automatically rolls back on error.
pub fn update_task_with_audit_log(
    repo: &WitTaskRepository,
    task_id: u64,
    new_status: &str,
    changed_by: &str,
) -> Result<(), TransactionError> {
    repo.with_transaction(|tx| {
        // Update task status
        let affected = tx.execute(
            "UPDATE tasks SET status = ? WHERE id = ?",
            &vec![new_status.to_string(), task_id.to_string()],
        )?;

        if affected == 0 {
            // Task not found - return error, transaction will rollback
            return Err(TransactionError::BeginError(format!(
                "Task {task_id} not found"
            )));
        }

        // Insert audit log
        tx.execute(
            "INSERT INTO task_audit_log (task_id, action, changed_by, changed_at) \
             VALUES (?, ?, ?, datetime('now'))",
            &vec![
                task_id.to_string(),
                format!("status_changed_to_{}", new_status),
                changed_by.to_string(),
            ],
        )?;

        Ok(())
    })
}

// =============================================================================
// Example 4: Nested Operations and Rollback
// =============================================================================

/// Demonstrates automatic rollback on error.
///
/// If any operation fails, all previous operations in the transaction
/// are automatically rolled back.
pub fn demonstrate_rollback_on_error() -> Result<(), TransactionError> {
    let repo = WitTaskRepository::new();

    let result = repo.with_transaction(|tx| {
        // First operation succeeds
        tx.execute(
            "INSERT INTO tasks (content, status) VALUES (?, ?)",
            &vec!["Task 1".to_string(), "Pending".to_string()],
        )?;

        // Second operation fails (intentionally for demo)
        // This will cause the transaction to rollback
        tx.execute(
            "INSERT INTO nonexistent_table (column) VALUES (?)",
            &vec!["value".to_string()],
        )?;

        Ok(())
    });

    // The first INSERT is rolled back because the second failed
    assert!(result.is_err());

    Ok(())
}

// =============================================================================
// Example 5: Transaction State Inspection
// =============================================================================

/// Shows how to inspect transaction state during operations.
pub fn inspect_transaction_state() -> Result<(), TransactionError> {
    let mut tx = Transaction::begin()?;

    assert!(tx.is_active());
    assert!(!tx.is_committed());
    assert!(!tx.is_rolled_back());

    // Perform some operations...
    tx.execute(
        "INSERT INTO tasks (content, status) VALUES (?, ?)",
        &vec!["Task".to_string(), "Pending".to_string()],
    )?;

    assert!(tx.is_active()); // Still active

    tx.commit()?;

    // Note: After commit(), tx is consumed and can no longer be used
    // The state inspection would need to happen before commit/rollback

    Ok(())
}

// =============================================================================
// Example 6: Complex Multi-Step Operation
// =============================================================================

/// Creates a merge request with associated metadata atomically.
///
/// This is a realistic example of a complex operation that benefits
/// from transaction support.
pub fn create_merge_request_with_metadata(
    repo: &WitBranchRepository,
    branch_id: &str,
    parent_id: Option<&str>,
    strategy: &str,
    approvers: &[String],
) -> Result<String, TransactionError> {
    repo.with_transaction(|tx| {
        // Generate merge request ID
        let merge_id = uuid::Uuid::new_v4().to_string();

        // Insert merge request
        tx.execute(
            "INSERT INTO merge_queue (id, branch_id, parent_id, strategy, status, created_at) \
             VALUES (?, ?, ?, ?, ?, datetime('now'))",
            &vec![
                merge_id.clone(),
                branch_id.to_string(),
                parent_id.unwrap_or("NULL").to_string(),
                strategy.to_string(),
                "pending".to_string(),
            ],
        )?;

        // Insert approver records
        for approver in approvers {
            tx.execute(
                "INSERT INTO merge_approvers (merge_id, approver, status) VALUES (?, ?, ?)",
                &vec![merge_id.clone(), approver.clone(), "pending".to_string()],
            )?;
        }

        // Update branch status to indicate merge in progress
        tx.execute(
            "UPDATE branches SET status_json = ? WHERE id = ?",
            &vec![
                r#"{"status": "merging"}"#.to_string(),
                branch_id.to_string(),
            ],
        )?;

        Ok(merge_id)
    })
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transaction_error_display() {
        let err = TransactionError::BeginError("connection failed".to_string());
        assert!(err.to_string().contains("connection failed"));

        let err = TransactionError::AlreadyCompleted;
        assert!(err.to_string().contains("already completed"));
    }

    #[test]
    fn transaction_state_checks() {
        // Note: This test would require a database connection to actually run
        // For now, just verify the API compiles correctly
        let _ = inspect_transaction_state;
    }
}
