//! Extended tests for the VFS (Virtual File System) module.

use brio_kernel::infrastructure::config::SandboxSettings;
use brio_kernel::vfs::manager::SessionManager;
use std::fs;

// =============================================================================
// Session Manager Tests
// =============================================================================

#[test]
fn begin_session_should_return_error_for_nonexistent_path() -> anyhow::Result<()> {
    let mut manager = SessionManager::new(&SandboxSettings::default())?;
    let result = manager.begin_session("/nonexistent/path/that/does/not/exist");

    assert!(result.is_err());
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("Invalid base path")
    );
    Ok(())
}

#[test]
fn commit_session_should_return_error_for_invalid_session_id() -> anyhow::Result<()> {
    let mut manager = SessionManager::new(&SandboxSettings::default())?;
    let result = manager.commit_session("fake-session-id-12345");

    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("not found"));
    Ok(())
}

#[test]
fn session_should_handle_empty_directories_correctly() -> anyhow::Result<()> {
    let temp = std::env::temp_dir().join("brio_vfs_test_empty");
    if temp.exists() {
        fs::remove_dir_all(&temp)?;
    }
    fs::create_dir_all(&temp)?;

    let mut manager = SessionManager::new(&SandboxSettings::default())?;
    let session_id = manager.begin_session(
        temp.to_str()
            .ok_or_else(|| anyhow::anyhow!("Invalid temp dir"))?,
    )?;

    // Session should be created even for empty directory
    let session_path = std::env::temp_dir().join("brio").join(&session_id);
    assert!(session_path.exists());

    // Commit should succeed with no changes
    let result = manager.commit_session(&session_id);
    assert!(result.is_ok());

    // Cleanup
    let _ = fs::remove_dir_all(&temp);
    Ok(())
}

#[test]
fn session_should_preserve_nested_directory_structure() -> anyhow::Result<()> {
    let temp = std::env::temp_dir().join("brio_vfs_test_nested");
    if temp.exists() {
        fs::remove_dir_all(&temp)?;
    }

    // Create nested directory structure
    fs::create_dir_all(temp.join("a/b/c"))?;
    fs::write(temp.join("root.txt"), "root")?;
    fs::write(temp.join("a/level1.txt"), "level1")?;
    fs::write(temp.join("a/b/level2.txt"), "level2")?;
    fs::write(temp.join("a/b/c/level3.txt"), "level3")?;

    let mut manager = SessionManager::new(&SandboxSettings::default())?;
    let session_id = manager.begin_session(
        temp.to_str()
            .ok_or_else(|| anyhow::anyhow!("Invalid temp dir"))?,
    )?;

    let session_path = std::env::temp_dir().join("brio").join(&session_id);

    // Verify all files exist in session
    assert!(session_path.join("root.txt").exists());
    assert!(session_path.join("a/level1.txt").exists());
    assert!(session_path.join("a/b/level2.txt").exists());
    assert!(session_path.join("a/b/c/level3.txt").exists());

    // Verify content
    assert_eq!(
        fs::read_to_string(session_path.join("a/b/c/level3.txt"))?,
        "level3"
    );

    // Cleanup
    let _ = fs::remove_dir_all(&temp);
    let _ = fs::remove_dir_all(&session_path);
    Ok(())
}

#[test]
fn session_should_apply_modifications_on_commit() -> anyhow::Result<()> {
    let temp = std::env::temp_dir().join("brio_vfs_test_modify");
    if temp.exists() {
        fs::remove_dir_all(&temp)?;
    }
    fs::create_dir_all(&temp)?;
    fs::write(temp.join("file.txt"), "original")?;

    let mut manager = SessionManager::new(&SandboxSettings::default())?;
    let session_id = manager.begin_session(
        temp.to_str()
            .ok_or_else(|| anyhow::anyhow!("Invalid temp dir"))?,
    )?;

    let session_path = std::env::temp_dir().join("brio").join(&session_id);

    // Modify in session
    fs::write(session_path.join("file.txt"), "modified")?;

    // Commit
    manager.commit_session(&session_id)?;

    // Base should have the modified content
    assert_eq!(fs::read_to_string(temp.join("file.txt"))?, "modified");

    // Cleanup
    let _ = fs::remove_dir_all(&temp);
    Ok(())
}

#[test]
fn session_should_add_new_files_on_commit() -> anyhow::Result<()> {
    let temp = std::env::temp_dir().join("brio_vfs_test_add");
    if temp.exists() {
        fs::remove_dir_all(&temp)?;
    }
    fs::create_dir_all(&temp)?;

    let mut manager = SessionManager::new(&SandboxSettings::default())?;
    let session_id = manager.begin_session(
        temp.to_str()
            .ok_or_else(|| anyhow::anyhow!("Invalid temp dir"))?,
    )?;

    let session_path = std::env::temp_dir().join("brio").join(&session_id);

    // Add new file in session
    fs::write(session_path.join("new_file.txt"), "new content")?;

    // Commit
    manager.commit_session(&session_id)?;

    // New file should exist in base
    assert!(temp.join("new_file.txt").exists());
    assert_eq!(
        fs::read_to_string(temp.join("new_file.txt"))?,
        "new content"
    );

    // Cleanup
    let _ = fs::remove_dir_all(&temp);
    Ok(())
}

#[test]
fn session_should_delete_files_on_commit() -> anyhow::Result<()> {
    let temp = std::env::temp_dir().join("brio_vfs_test_delete");
    if temp.exists() {
        fs::remove_dir_all(&temp)?;
    }
    fs::create_dir_all(&temp)?;
    fs::write(temp.join("to_delete.txt"), "delete me")?;

    let mut manager = SessionManager::new(&SandboxSettings::default())?;
    let session_id = manager.begin_session(
        temp.to_str()
            .ok_or_else(|| anyhow::anyhow!("Invalid temp dir"))?,
    )?;

    let session_path = std::env::temp_dir().join("brio").join(&session_id);

    // Delete file in session
    fs::remove_file(session_path.join("to_delete.txt"))?;

    // Commit
    manager.commit_session(&session_id)?;

    // File should be deleted from base
    assert!(!temp.join("to_delete.txt").exists());

    // Cleanup
    let _ = fs::remove_dir_all(&temp);
    Ok(())
}

// =============================================================================
// SessionManager Default Trait Test
// =============================================================================

#[test]
fn session_manager_default_should_create_valid_instance() {
    let manager = SessionManager::default();
    // Just verify it can be created via Default trait
    drop(manager);
}
