//! Integration tests for Virtual File System operations.
//!
//! Tests session lifecycle, isolation, rollback, and sandbox policy enforcement
//! for the VFS subsystem.

use anyhow::Result;
use brio_kernel::infrastructure::config::SandboxSettings;
use std::io::BufRead;

mod common;

/// Test VFS session lifecycle (begin, modify, commit).
#[tokio::test]
async fn test_vfs_session_lifecycle() -> Result<()> {
    // Create test context
    let ctx = common::IntegrationTestContext::new().await?;
    let base_path = ctx.temp_path();

    // Create initial file in base directory
    let original_file = base_path.join("test.txt");
    std::fs::write(&original_file, "original content")?;

    // Begin session
    let session_id = ctx.host.begin_session(base_path.to_str().unwrap())?;
    let session_path = std::env::temp_dir().join("brio").join(&session_id);

    // Verify session was created
    assert!(session_path.exists(), "Session directory should exist");

    // Modify files in session
    let session_file = session_path.join("test.txt");
    std::fs::write(&session_file, "modified content")?;

    // Add new file in session
    let new_file = session_path.join("new.txt");
    std::fs::write(&new_file, "new file content")?;

    // Verify modifications are in session only
    let original_content = std::fs::read_to_string(&original_file)?;
    assert_eq!(
        original_content, "original content",
        "Original file should be unchanged"
    );

    // Commit session
    ctx.host.commit_session(&session_id)?;

    // Assert: Changes persisted
    let committed_content = std::fs::read_to_string(&original_file)?;
    assert_eq!(
        committed_content, "modified content",
        "Changes should be committed"
    );
    assert!(
        new_file.exists() || base_path.join("new.txt").exists(),
        "New file should exist after commit"
    );

    // Verify session cleaned up
    assert!(
        !session_path.exists(),
        "Session directory should be cleaned up"
    );

    Ok(())
}

/// Test VFS session isolation between multiple sessions.
#[tokio::test]
async fn test_vfs_session_isolation() -> Result<()> {
    let ctx = common::IntegrationTestContext::new().await?;
    let base_path = ctx.temp_path();

    // Create initial file
    let original_file = base_path.join("shared.txt");
    std::fs::write(&original_file, "base content")?;

    // Create two sessions
    let session1_id = ctx.host.begin_session(base_path.to_str().unwrap())?;
    let session2_id = ctx.host.begin_session(base_path.to_str().unwrap())?;

    let session1_path = std::env::temp_dir().join("brio").join(&session1_id);
    let session2_path = std::env::temp_dir().join("brio").join(&session2_id);

    // Modify in session 1
    let session1_file = session1_path.join("shared.txt");
    std::fs::write(&session1_file, "session 1 content")?;

    // Modify in session 2
    let session2_file = session2_path.join("shared.txt");
    std::fs::write(&session2_file, "session 2 content")?;

    // Assert: Session 2 doesn't see session 1 changes
    let session2_content = std::fs::read_to_string(&session2_file)?;
    assert_eq!(
        session2_content, "session 2 content",
        "Session 2 should have its own changes"
    );

    // Original should be unchanged
    let original_content = std::fs::read_to_string(&original_file)?;
    assert_eq!(
        original_content, "base content",
        "Original should remain unchanged"
    );

    // Cleanup
    ctx.host.rollback_session(&session1_id)?;
    ctx.host.rollback_session(&session2_id)?;

    Ok(())
}

/// Test VFS session rollback functionality.
#[tokio::test]
async fn test_vfs_session_rollback() -> Result<()> {
    let ctx = common::IntegrationTestContext::new().await?;
    let base_path = ctx.temp_path();

    // Create initial file
    let original_file = base_path.join("data.txt");
    std::fs::write(&original_file, "original")?;

    // Begin session
    let session_id = ctx.host.begin_session(base_path.to_str().unwrap())?;
    let session_path = std::env::temp_dir().join("brio").join(&session_id);

    // Modify files
    let session_file = session_path.join("data.txt");
    std::fs::write(&session_file, "modified")?;

    // Add new file
    let new_file = session_path.join("temp.txt");
    std::fs::write(&new_file, "temporary")?;

    // Rollback session
    ctx.host.rollback_session(&session_id)?;

    // Assert: Changes reverted
    let reverted_content = std::fs::read_to_string(&original_file)?;
    assert_eq!(
        reverted_content, "original",
        "Original content should be preserved"
    );

    // New file should not exist in base
    let new_in_base = base_path.join("temp.txt");
    assert!(
        !new_in_base.exists(),
        "New file should not exist after rollback"
    );

    // Session directory should be cleaned up
    assert!(
        !session_path.exists(),
        "Session directory should be cleaned up after rollback"
    );

    Ok(())
}

/// Test VFS sandbox policy enforcement.
#[tokio::test]
async fn test_vfs_sandbox_policy() -> Result<()> {
    // Create restricted sandbox with a temporary allowed path
    let mut sandbox = SandboxSettings::default();
    let temp_allowed = std::env::temp_dir()
        .join("brio_test_")
        .join(format!("{}", std::process::id()));
    std::fs::create_dir_all(&temp_allowed)?;
    sandbox.allowed_paths = vec![temp_allowed.to_str().unwrap().to_string()];

    let ctx = common::IntegrationTestContext::with_sandbox(sandbox).await?;

    // Try to begin session outside sandbox
    let outside_path = "/etc";
    let result = ctx.host.begin_session(outside_path);

    // Assert: Access denied
    assert!(
        result.is_err(),
        "Should deny access to path outside sandbox"
    );

    // Verify the error type
    if let Err(e) = result {
        let err_str = e.to_string();
        assert!(
            err_str.contains("PolicyViolation") || err_str.contains("policy"),
            "Error should indicate policy violation: {}",
            err_str
        );
    }

    Ok(())
}

/// Test VFS conflict detection on commit.
#[tokio::test]
async fn test_vfs_conflict_detection() -> Result<()> {
    let ctx = common::IntegrationTestContext::new().await?;
    let base_path = ctx.temp_path();

    // Create initial file
    let original_file = base_path.join("data.txt");
    std::fs::write(&original_file, "original")?;

    // Begin session
    let session_id = ctx.host.begin_session(base_path.to_str().unwrap())?;
    let session_path = std::env::temp_dir().join("brio").join(&session_id);

    // Modify in session
    let session_file = session_path.join("data.txt");
    std::fs::write(&session_file, "modified in session")?;

    // Modify base (simulating external change)
    std::fs::write(&original_file, "modified externally")?;

    // Try to commit - should detect conflict
    let result = ctx.host.commit_session(&session_id);

    // Assert: Conflict detected
    assert!(
        result.is_err(),
        "Should detect conflict when base is modified"
    );

    if let Err(e) = result {
        let err_str = e.to_string();
        assert!(
            err_str.contains("Conflict") || err_str.contains("conflict"),
            "Error should indicate conflict: {}",
            err_str
        );
    }

    Ok(())
}

/// Test VFS concurrent session handling.
#[tokio::test]
async fn test_vfs_concurrent_sessions() -> Result<()> {
    let ctx = common::IntegrationTestContext::new().await?;
    let base_path = ctx.temp_path();

    // Create initial file
    let test_file = base_path.join("concurrent.txt");
    std::fs::write(&test_file, "base")?;

    // Create multiple concurrent sessions
    let mut session_ids = Vec::new();
    for i in 0..5 {
        let session_id = ctx.host.begin_session(base_path.to_str().unwrap())?;
        let session_path = std::env::temp_dir().join("brio").join(&session_id);

        // Each session modifies the file
        let session_file = session_path.join("concurrent.txt");
        std::fs::write(&session_file, format!("session {}", i))?;

        session_ids.push(session_id);
    }

    // Commit one session
    let commit_id = session_ids.remove(0);
    ctx.host.commit_session(&commit_id)?;

    // Rollback others
    for id in session_ids {
        ctx.host.rollback_session(&id)?;
    }

    // Assert: Only committed changes remain
    let final_content = std::fs::read_to_string(&test_file)?;
    assert_eq!(
        final_content, "session 0",
        "Only committed session changes should remain"
    );

    Ok(())
}
