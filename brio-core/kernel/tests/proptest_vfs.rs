//! Property-based tests for VFS session lifecycle.
//!
//! Uses proptest to generate random file operations and verify that
//! the session manager correctly applies changes to the base directory.

use brio_kernel::vfs::manager::SessionManager;
use proptest::prelude::*;
use std::fs;

/// Strategy to generate valid file names (no special chars, reasonable length)
fn file_name_strategy() -> impl Strategy<Value = String> {
    "[a-z][a-z0-9]{0,7}\\.txt".prop_filter("Valid filename", |s| !s.is_empty())
}

/// Strategy to generate file content
fn content_strategy() -> impl Strategy<Value = String> {
    "[a-zA-Z0-9 ]{1,50}"
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(20))]

    /// Property: After creating files in a session and committing,
    /// all created files should exist in the base directory.
    #[test]
    fn created_files_appear_in_base_after_commit(
        files in prop::collection::vec((file_name_strategy(), content_strategy()), 1..3)
    ) {
        // Setup
        let test_id = uuid::Uuid::new_v4().to_string();
        let base = std::env::temp_dir().join(format!("proptest_vfs_create_{}", test_id));
        if base.exists() {
            fs::remove_dir_all(&base).unwrap();
        }
        fs::create_dir_all(&base).unwrap();

        let mut manager = SessionManager::new();
        let session_id = manager.begin_session(base.to_str().unwrap().to_string()).unwrap();
        let session_path = std::env::temp_dir().join("brio").join(&session_id);

        // Create files in session
        for (name, content) in &files {
            fs::write(session_path.join(name), content).unwrap();
        }

        // Commit
        manager.commit_session(session_id).unwrap();

        // Verify all files exist in base
        for (name, content) in &files {
            let file_path = base.join(name);
            prop_assert!(file_path.exists(), "File {} should exist in base", name);
            prop_assert_eq!(fs::read_to_string(&file_path).unwrap(), content.clone());
        }

        // Cleanup
        let _ = fs::remove_dir_all(&base);
    }

    /// Property: Deleting files in a session removes them from base after commit.
    #[test]
    fn deleted_files_removed_from_base_after_commit(
        files_to_delete in prop::collection::vec(file_name_strategy(), 1..3)
    ) {
        // Setup
        let test_id = uuid::Uuid::new_v4().to_string();
        let base = std::env::temp_dir().join(format!("proptest_vfs_delete_{}", test_id));
        if base.exists() {
            fs::remove_dir_all(&base).unwrap();
        }
        fs::create_dir_all(&base).unwrap();

        // Create files in base first
        let unique_names: Vec<_> = files_to_delete.iter()
            .enumerate()
            .map(|(i, name)| format!("{}_{}", name, i))
            .collect();

        for name in &unique_names {
            fs::write(base.join(name), "to be deleted").unwrap();
        }

        let mut manager = SessionManager::new();
        let session_id = manager.begin_session(base.to_str().unwrap().to_string()).unwrap();
        let session_path = std::env::temp_dir().join("brio").join(&session_id);

        // Delete files in session
        for name in &unique_names {
            let file_path = session_path.join(name);
            if file_path.exists() {
                fs::remove_file(&file_path).unwrap();
            }
        }

        // Commit
        manager.commit_session(session_id).unwrap();

        // Verify all files are deleted from base
        for name in &unique_names {
            prop_assert!(!base.join(name).exists(), "File {} should be deleted from base", name);
        }

        // Cleanup
        let _ = fs::remove_dir_all(&base);
    }

    /// Property: Modifying files in a session updates them in base after commit.
    #[test]
    fn modified_files_updated_in_base_after_commit(
        modifications in prop::collection::vec(
            (file_name_strategy(), content_strategy()),
            1..3
        )
    ) {
        // Setup
        let test_id = uuid::Uuid::new_v4().to_string();
        let base = std::env::temp_dir().join(format!("proptest_vfs_modify_{}", test_id));
        if base.exists() {
            fs::remove_dir_all(&base).unwrap();
        }
        fs::create_dir_all(&base).unwrap();

        // Create files in base with original content
        let unique_mods: Vec<_> = modifications.iter()
            .enumerate()
            .map(|(i, (name, content))| (format!("{}_{}.txt", name.trim_end_matches(".txt"), i), content.clone()))
            .collect();

        for (name, _) in &unique_mods {
            fs::write(base.join(name), "original content").unwrap();
        }

        let mut manager = SessionManager::new();
        let session_id = manager.begin_session(base.to_str().unwrap().to_string()).unwrap();
        let session_path = std::env::temp_dir().join("brio").join(&session_id);

        // Modify files in session
        for (name, new_content) in &unique_mods {
            fs::write(session_path.join(name), new_content).unwrap();
        }

        // Commit
        manager.commit_session(session_id).unwrap();

        // Verify all files have new content
        for (name, expected_content) in &unique_mods {
            let actual = fs::read_to_string(base.join(name)).unwrap();
            prop_assert_eq!(actual, expected_content.clone(), "File {} should have updated content", name);
        }

        // Cleanup
        let _ = fs::remove_dir_all(&base);
    }
}
