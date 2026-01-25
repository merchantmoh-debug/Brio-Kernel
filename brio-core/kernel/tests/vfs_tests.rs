//! Extended tests for the VFS (Virtual File System) module.

use brio_kernel::vfs::manager::SessionManager;
use std::fs;

// =============================================================================
// Session Manager Tests
// =============================================================================

#[test]
fn test_session_begin_with_nonexistent_path() {
    let mut manager = SessionManager::new();
    let result = manager.begin_session("/nonexistent/path/that/does/not/exist".to_string());

    assert!(result.is_err());
    assert!(result.unwrap_err().contains("does not exist"));
}

#[test]
fn test_commit_nonexistent_session() {
    let mut manager = SessionManager::new();
    let result = manager.commit_session("fake-session-id-12345".to_string());

    assert!(result.is_err());
    assert!(result.unwrap_err().contains("not found"));
}

#[test]
fn test_session_with_empty_directory() {
    let temp = std::env::temp_dir().join("brio_vfs_test_empty");
    if temp.exists() {
        fs::remove_dir_all(&temp).unwrap();
    }
    fs::create_dir_all(&temp).unwrap();

    let mut manager = SessionManager::new();
    let session_id = manager
        .begin_session(temp.to_str().unwrap().to_string())
        .unwrap();

    // Session should be created even for empty directory
    let session_path = std::env::temp_dir().join("brio").join(&session_id);
    assert!(session_path.exists());

    // Commit should succeed with no changes
    let result = manager.commit_session(session_id);
    assert!(result.is_ok());

    // Cleanup
    let _ = fs::remove_dir_all(&temp);
}

#[test]
fn test_session_preserves_nested_structure() {
    let temp = std::env::temp_dir().join("brio_vfs_test_nested");
    if temp.exists() {
        fs::remove_dir_all(&temp).unwrap();
    }

    // Create nested directory structure
    fs::create_dir_all(temp.join("a/b/c")).unwrap();
    fs::write(temp.join("root.txt"), "root").unwrap();
    fs::write(temp.join("a/level1.txt"), "level1").unwrap();
    fs::write(temp.join("a/b/level2.txt"), "level2").unwrap();
    fs::write(temp.join("a/b/c/level3.txt"), "level3").unwrap();

    let mut manager = SessionManager::new();
    let session_id = manager
        .begin_session(temp.to_str().unwrap().to_string())
        .unwrap();

    let session_path = std::env::temp_dir().join("brio").join(&session_id);

    // Verify all files exist in session
    assert!(session_path.join("root.txt").exists());
    assert!(session_path.join("a/level1.txt").exists());
    assert!(session_path.join("a/b/level2.txt").exists());
    assert!(session_path.join("a/b/c/level3.txt").exists());

    // Verify content
    assert_eq!(
        fs::read_to_string(session_path.join("a/b/c/level3.txt")).unwrap(),
        "level3"
    );

    // Cleanup
    let _ = fs::remove_dir_all(&temp);
    let _ = fs::remove_dir_all(&session_path);
}

#[test]
fn test_session_modification_and_commit() {
    let temp = std::env::temp_dir().join("brio_vfs_test_modify");
    if temp.exists() {
        fs::remove_dir_all(&temp).unwrap();
    }
    fs::create_dir_all(&temp).unwrap();
    fs::write(temp.join("file.txt"), "original").unwrap();

    let mut manager = SessionManager::new();
    let session_id = manager
        .begin_session(temp.to_str().unwrap().to_string())
        .unwrap();

    let session_path = std::env::temp_dir().join("brio").join(&session_id);

    // Modify in session
    fs::write(session_path.join("file.txt"), "modified").unwrap();

    // Commit
    manager.commit_session(session_id).unwrap();

    // Base should have the modified content
    assert_eq!(
        fs::read_to_string(temp.join("file.txt")).unwrap(),
        "modified"
    );

    // Cleanup
    let _ = fs::remove_dir_all(&temp);
}

#[test]
fn test_session_add_new_file() {
    let temp = std::env::temp_dir().join("brio_vfs_test_add");
    if temp.exists() {
        fs::remove_dir_all(&temp).unwrap();
    }
    fs::create_dir_all(&temp).unwrap();

    let mut manager = SessionManager::new();
    let session_id = manager
        .begin_session(temp.to_str().unwrap().to_string())
        .unwrap();

    let session_path = std::env::temp_dir().join("brio").join(&session_id);

    // Add new file in session
    fs::write(session_path.join("new_file.txt"), "new content").unwrap();

    // Commit
    manager.commit_session(session_id).unwrap();

    // New file should exist in base
    assert!(temp.join("new_file.txt").exists());
    assert_eq!(
        fs::read_to_string(temp.join("new_file.txt")).unwrap(),
        "new content"
    );

    // Cleanup
    let _ = fs::remove_dir_all(&temp);
}

#[test]
fn test_session_delete_file() {
    let temp = std::env::temp_dir().join("brio_vfs_test_delete");
    if temp.exists() {
        fs::remove_dir_all(&temp).unwrap();
    }
    fs::create_dir_all(&temp).unwrap();
    fs::write(temp.join("to_delete.txt"), "delete me").unwrap();

    let mut manager = SessionManager::new();
    let session_id = manager
        .begin_session(temp.to_str().unwrap().to_string())
        .unwrap();

    let session_path = std::env::temp_dir().join("brio").join(&session_id);

    // Delete file in session
    fs::remove_file(session_path.join("to_delete.txt")).unwrap();

    // Commit
    manager.commit_session(session_id).unwrap();

    // File should be deleted from base
    assert!(!temp.join("to_delete.txt").exists());

    // Cleanup
    let _ = fs::remove_dir_all(&temp);
}

// =============================================================================
// SessionManager Default Trait Test
// =============================================================================

#[test]
fn test_session_manager_default() {
    let manager = SessionManager::default();
    // Just verify it can be created via Default trait
    drop(manager);
}
