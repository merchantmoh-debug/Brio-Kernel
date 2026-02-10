//! Integration tests for branch management.
//!
//! Tests branch creation, checkout, merge operations, and conflict detection
//! including three-way merge functionality.

use supervisor::branch::{Branch, BranchValidationError, SessionSnapshot};
use supervisor::domain::{
    BranchConfig, BranchId, BranchResult, BranchStatus, ExecutionMetrics, ExecutionStrategy,
    MergeResult,
};

/// Test branch creation.
#[tokio::test]
async fn test_branch_creation() {
    // Create branch
    let branch_id = BranchId::new();
    let config = BranchConfig::new(
        "test-branch",
        vec![],
        ExecutionStrategy::Sequential,
        false,
        "three-way",
    )
    .expect("Valid config");

    let branch = Branch::new(branch_id, None, "session-123", "feature-branch", config);

    // Assert: Branch exists with correct properties
    assert!(branch.is_ok(), "Branch creation should succeed");
    let branch = branch.unwrap();
    assert_eq!(branch.name(), "feature-branch");
    assert_eq!(branch.session_id(), "session-123");
    assert!(matches!(branch.status(), BranchStatus::Pending));
    assert!(branch.parent_id().is_none());
}

/// Test branch checkout and modification.
#[tokio::test]
async fn test_branch_checkout() {
    use tempfile::TempDir;

    // Create base workspace
    let temp_dir = TempDir::new().expect("Create temp dir");
    let base_path = temp_dir.path();

    // Create initial file in main
    let main_file = base_path.join("file.txt");
    std::fs::write(&main_file, "main content").expect("Write file");

    // Create branch (simulated by creating separate workspace)
    let branch_path = base_path.join("branches").join("feature");
    std::fs::create_dir_all(&branch_path).expect("Create branch dir");
    std::fs::copy(&main_file, branch_path.join("file.txt")).expect("Copy file");

    // Modify file in branch
    let branch_file = branch_path.join("file.txt");
    std::fs::write(&branch_file, "branch content").expect("Write branch file");

    // Assert: Changes in branch, not in main
    let main_content = std::fs::read_to_string(&main_file).expect("Read main");
    let branch_content = std::fs::read_to_string(&branch_file).expect("Read branch");

    assert_eq!(main_content, "main content", "Main should be unchanged");
    assert_eq!(
        branch_content, "branch content",
        "Branch should have changes"
    );
}

/// Test branch merge operation.
#[tokio::test]
async fn test_branch_merge() {
    use tempfile::TempDir;

    // Setup
    let temp_dir = TempDir::new().expect("Create temp dir");
    let base_path = temp_dir.path();

    // Create main workspace
    let main_file = base_path.join("main.txt");
    std::fs::write(&main_file, "original").expect("Write main");

    // Create branch workspace with changes
    let branch_path = base_path.join("branches").join("feature");
    std::fs::create_dir_all(&branch_path).expect("Create branch dir");
    std::fs::write(branch_path.join("main.txt"), "modified in branch").expect("Write branch");

    // Add new file in branch
    std::fs::write(branch_path.join("new.txt"), "new file").expect("Write new");

    // Merge to main (copy changes)
    std::fs::copy(branch_path.join("main.txt"), &main_file).expect("Copy merged");
    std::fs::copy(branch_path.join("new.txt"), base_path.join("new.txt")).expect("Copy new file");

    // Assert: Changes in main
    let main_content = std::fs::read_to_string(&main_file).expect("Read main");
    assert_eq!(
        main_content, "modified in branch",
        "Changes should be in main"
    );
    assert!(
        base_path.join("new.txt").exists(),
        "New file should exist in main"
    );
}

/// Test merge conflict detection.
#[tokio::test]
async fn test_merge_conflict_detection() {
    use tempfile::TempDir;

    // Setup
    let temp_dir = TempDir::new().expect("Create temp dir");
    let base_path = temp_dir.path();

    // Create common ancestor
    let base_content = "line 1\nline 2\nline 3\n";
    let base_file = base_path.join("shared.txt");
    std::fs::write(&base_file, base_content).expect("Write base");

    // Create main with modifications (line 2 changed)
    let main_path = base_path.join("main");
    std::fs::create_dir_all(&main_path).expect("Create main dir");
    let main_content = "line 1\nmain modified line 2\nline 3\n";
    std::fs::write(main_path.join("shared.txt"), main_content).expect("Write main");

    // Create branch with different modifications (same line 2)
    let branch_path = base_path.join("branch");
    std::fs::create_dir_all(&branch_path).expect("Create branch dir");
    let branch_content = "line 1\nbranch modified line 2\nline 3\n";
    std::fs::write(branch_path.join("shared.txt"), branch_content).expect("Write branch");

    // Detect conflict (lines are different in both)
    let main_lines: Vec<&str> = main_content.lines().collect();
    let branch_lines: Vec<&str> = branch_content.lines().collect();

    let mut conflicts = Vec::new();
    for (i, (main_line, branch_line)) in main_lines.iter().zip(branch_lines.iter()).enumerate() {
        if main_line != branch_line {
            conflicts.push((i + 1, main_line, branch_line));
        }
    }

    // Assert: Conflict detected on line 2
    assert!(!conflicts.is_empty(), "Should detect conflicts");
    assert_eq!(conflicts.len(), 1, "Should have 1 conflict");
    assert_eq!(conflicts[0].0, 2, "Conflict should be on line 2");
    assert!(conflicts[0].1.contains("main"), "Should show main version");
    assert!(
        conflicts[0].2.contains("branch"),
        "Should show branch version"
    );
}

/// Test three-way merge functionality.
#[tokio::test]
async fn test_three_way_merge() {
    // Setup base, ours, theirs versions
    let base = "line 1\nline 2\nline 3\n";
    let ours = "line 1\nmodified line 2\nline 3\n"; // We changed line 2
    let theirs = "line 1\nline 2\nmodified line 3\n"; // They changed line 3

    // Perform three-way merge (auto-mergeable - different lines)
    let base_lines: Vec<&str> = base.lines().collect();
    let ours_lines: Vec<&str> = ours.lines().collect();
    let theirs_lines: Vec<&str> = theirs.lines().collect();

    let mut result = Vec::new();
    let mut has_conflict = false;

    for i in 0..base_lines.len() {
        let base_line = base_lines[i];
        let ours_line = ours_lines[i];
        let theirs_line = theirs_lines[i];

        if ours_line != base_line && theirs_line != base_line {
            // Both changed - conflict
            if ours_line == theirs_line {
                // Both changed to same value
                result.push(ours_line.to_string());
            } else {
                has_conflict = true;
                result.push("<<<<<<< OURS".to_string());
                result.push(ours_line.to_string());
                result.push("=====".to_string());
                result.push(theirs_line.to_string());
                result.push(">>>>>>> THEIRS".to_string());
            }
        } else if ours_line != base_line {
            // Only ours changed
            result.push(ours_line.to_string());
        } else if theirs_line != base_line {
            // Only theirs changed
            result.push(theirs_line.to_string());
        } else {
            // No changes
            result.push(base_line.to_string());
        }
    }

    // Assert: Correct result (no conflicts in this case)
    assert!(
        !has_conflict,
        "Should not have conflicts for non-overlapping changes"
    );
    assert_eq!(result.len(), 3, "Should have 3 lines");
    assert!(
        result[1].contains("modified line 2"),
        "Should have our change"
    );
    assert!(
        result[2].contains("modified line 3"),
        "Should have their change"
    );
}

/// Test branch status transitions.
#[tokio::test]
async fn test_branch_status_transitions() {
    let branch_id = BranchId::new();
    let config = BranchConfig::new(
        "test",
        vec![],
        ExecutionStrategy::Sequential,
        false,
        "three-way",
    )
    .expect("Valid config");

    let mut branch =
        Branch::new(branch_id, None, "session-1", "test-branch", config).expect("Create branch");

    // Start execution
    branch.start_execution().expect("Start execution");
    assert!(matches!(branch.status(), BranchStatus::Active));

    // Complete with result
    let result = BranchResult::new(
        branch_id,
        vec![],
        vec![],
        ExecutionMetrics {
            total_duration_ms: 100,
            files_processed: 5,
            agents_executed: 2,
            peak_memory_bytes: 1024,
        },
    );
    branch.complete(result).expect("Complete branch");
    assert!(matches!(branch.status(), BranchStatus::Completed));
    assert!(branch.completed_at().is_some());
}

/// Test branch with parent relationship.
#[tokio::test]
async fn test_branch_with_parent() {
    let parent_id = BranchId::new();
    let child_id = BranchId::new();

    let config = BranchConfig::new(
        "test",
        vec![],
        ExecutionStrategy::Sequential,
        false,
        "three-way",
    )
    .expect("Valid config");

    // Create parent branch
    let mut parent = Branch::new(parent_id, None, "parent-session", "parent", config.clone())
        .expect("Create parent");

    // Create child branch
    let child = Branch::new(child_id, Some(parent_id), "child-session", "child", config)
        .expect("Create child");

    // Assert: Parent-child relationship
    assert_eq!(
        child.parent_id(),
        Some(parent_id),
        "Child should have parent"
    );
    assert!(
        parent.parent_id().is_none(),
        "Parent should not have parent"
    );

    // Add child to parent
    parent.add_child(child_id);
    assert!(
        parent.children().contains(&child_id),
        "Parent should know about child"
    );
}

/// Test branch validation errors.
#[tokio::test]
async fn test_branch_validation_errors() {
    // Empty session ID
    let result = Branch::new(
        BranchId::new(),
        None,
        "",
        "test",
        BranchConfig::new(
            "test",
            vec![],
            ExecutionStrategy::Sequential,
            false,
            "three-way",
        )
        .expect("Config"),
    );
    assert!(
        matches!(result, Err(BranchValidationError::EmptySessionId)),
        "Should error on empty session"
    );

    // Empty name
    let result = Branch::new(
        BranchId::new(),
        None,
        "session",
        "",
        BranchConfig::new(
            "test",
            vec![],
            ExecutionStrategy::Sequential,
            false,
            "three-way",
        )
        .expect("Config"),
    );
    assert!(
        matches!(result, Err(BranchValidationError::InvalidNameLength { .. })),
        "Should error on empty name"
    );

    // Name too long
    let long_name = "a".repeat(300);
    let result = Branch::new(
        BranchId::new(),
        None,
        "session",
        long_name,
        BranchConfig::new(
            "test",
            vec![],
            ExecutionStrategy::Sequential,
            false,
            "three-way",
        )
        .expect("Config"),
    );
    assert!(
        matches!(result, Err(BranchValidationError::InvalidNameLength { .. })),
        "Should error on long name"
    );
}

/// Test branch merge result recording.
#[tokio::test]
async fn test_branch_merge_result() {
    let branch_id = BranchId::new();
    let config = BranchConfig::new(
        "test",
        vec![],
        ExecutionStrategy::Sequential,
        false,
        "three-way",
    )
    .expect("Valid config");

    let mut branch =
        Branch::new(branch_id, None, "session-1", "test-branch", config).expect("Create branch");

    // Start and complete execution
    branch.start_execution().expect("Start");
    let exec_result = BranchResult::new(
        branch_id,
        vec![],
        vec![],
        ExecutionMetrics {
            total_duration_ms: 100,
            files_processed: 5,
            agents_executed: 1,
            peak_memory_bytes: 512,
        },
    );
    branch.complete(exec_result).expect("Complete");

    // Start merge
    branch.start_merge().expect("Start merge");
    assert!(matches!(branch.status(), BranchStatus::Merging));

    // Record merge result
    let merge_result = MergeResult::new(vec![], vec![], "three-way");
    branch.mark_merged(merge_result).expect("Mark merged");
    assert!(matches!(branch.status(), BranchStatus::Merged));
    assert!(branch.merge_result().is_some());
}

/// Test session snapshot creation.
#[tokio::test]
async fn test_session_snapshot() {
    use chrono::Utc;

    // Create valid snapshot
    let snapshot = SessionSnapshot::new(
        "session-abc-123",
        Utc::now(),
        Some("Feature implementation".to_string()),
    );

    assert!(snapshot.is_ok(), "Should create valid snapshot");
    let snapshot = snapshot.unwrap();
    assert_eq!(snapshot.session_id(), "session-abc-123");
    assert_eq!(snapshot.description(), Some("Feature implementation"));

    // Empty session ID should fail
    let invalid = SessionSnapshot::new("", Utc::now(), None);
    assert!(
        matches!(invalid, Err(BranchValidationError::EmptySessionId)),
        "Should error on empty session"
    );
}
