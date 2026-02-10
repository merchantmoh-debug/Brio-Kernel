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
//! - Integration with both `TaskRepository` and `BranchRepository`

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
///
/// # Errors
/// Returns `TransactionError` if the transaction fails to begin, execute, or commit.
pub fn create_task_with_subtasks_manual(
    content: String,
    subtask_contents: Vec<String>,
) -> Result<u64, TransactionError> {
    // Begin a new transaction
    let tx = Transaction::begin()?;

    // Insert the parent task
    let parent_result = tx.query(
        "INSERT INTO tasks (content, priority, status, parent_id) VALUES (?, ?, ?, ?) RETURNING id",
        &[
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
            &[
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
///
/// # Errors
/// Returns `TransactionError` if the transaction fails to execute or commit.
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
            &[
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
            &[branch_id.to_string(), initial_state_json.to_string()],
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
///
/// # Errors
/// Returns `TransactionError` if the transaction fails to execute or commit,
/// or if the task is not found.
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
            &[new_status.to_string(), task_id.to_string()],
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
            &[
                task_id.to_string(),
                format!("status_changed_to_{new_status}"),
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
///
/// # Errors
/// Returns `TransactionError` if the transaction operations fail.
///
/// # Panics
/// Panics if the rollback demonstration doesn't result in an error as expected.
pub fn demonstrate_rollback_on_error() -> Result<(), TransactionError> {
    let repo = WitTaskRepository::new();

    let result: Result<(), TransactionError> = repo.with_transaction(|tx| {
        // First operation succeeds
        tx.execute(
            "INSERT INTO tasks (content, status) VALUES (?, ?)",
            &["Task 1".to_string(), "Pending".to_string()],
        )?;

        // Second operation fails (intentionally for demo)
        // This will cause the transaction to rollback
        tx.execute(
            "INSERT INTO nonexistent_table (column) VALUES (?)",
            &["value".to_string()],
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
///
/// # Errors
/// Returns `TransactionError` if the transaction operations fail.
///
/// # Panics
/// Panics if transaction state assertions fail.
pub fn inspect_transaction_state() -> Result<(), TransactionError> {
    let tx = Transaction::begin()?;

    assert!(tx.is_active());
    assert!(!tx.is_committed());
    assert!(!tx.is_rolled_back());

    // Perform some operations...
    tx.execute(
        "INSERT INTO tasks (content, status) VALUES (?, ?)",
        &["Task".to_string(), "Pending".to_string()],
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
///
/// # Errors
/// Returns `TransactionError` if the transaction fails to execute or commit.
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
            &[
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
                &[merge_id.clone(), approver.clone(), "pending".to_string()],
            )?;
        }

        // Update branch status to indicate merge in progress
        tx.execute(
            "UPDATE branches SET status_json = ? WHERE id = ?",
            &[
                r#"{"status": "merging"}"#.to_string(),
                branch_id.to_string(),
            ],
        )?;

        Ok(merge_id)
    })
}

// =============================================================================
// Main - Entry point for the example
// =============================================================================

fn main() {
    println!("Transaction API Usage Examples");
    println!("==============================\n");

    // Example 1: Manual transaction control
    println!("Example 1: Manual Transaction Control");
    match create_task_with_subtasks_manual(
        "Parent task".to_string(),
        vec!["Subtask 1".to_string(), "Subtask 2".to_string()],
    ) {
        Ok(id) => println!("  Created task with ID: {id}"),
        Err(e) => println!("  Error: {e}"),
    }
    println!();

    // Example 2: Using with_transaction closure API
    println!("Example 2: with_transaction Closure API");
    let repo = WitBranchRepository::new();
    match create_branch_with_state_atomic(
        &repo,
        "branch-001",
        "session-001",
        "feature-branch",
        r#"{"status": "active"}"#,
    ) {
        Ok(()) => println!("  Branch created successfully"),
        Err(e) => println!("  Error: {e}"),
    }
    println!();

    // Example 3: Multi-Table Operations with Error Handling
    println!("Example 3: Update Task with Audit Log");
    let task_repo = WitTaskRepository::new();
    match update_task_with_audit_log(&task_repo, 1, "InProgress", "user@example.com") {
        Ok(()) => println!("  Task updated and logged successfully"),
        Err(e) => println!("  Error: {e}"),
    }
    println!();

    // Example 4: Demonstrate rollback on error
    println!("Example 4: Rollback on Error Demonstration");
    match demonstrate_rollback_on_error() {
        Ok(()) => println!("  Rollback demonstration completed"),
        Err(e) => println!("  Error: {e}"),
    }
    println!();

    // Example 5: Transaction state inspection
    println!("Example 5: Transaction State Inspection");
    match inspect_transaction_state() {
        Ok(()) => println!("  State inspection completed"),
        Err(e) => println!("  Error: {e}"),
    }
    println!();

    // Example 6: Complex multi-step operation
    println!("Example 6: Create Merge Request with Metadata");
    let approvers = vec![
        "alice@example.com".to_string(),
        "bob@example.com".to_string(),
    ];
    match create_merge_request_with_metadata(&repo, "branch-001", Some("main"), "merge", &approvers)
    {
        Ok(id) => println!("  Merge request created with ID: {id}"),
        Err(e) => println!("  Error: {e}"),
    }
    println!();

    println!("All examples completed!");
}
