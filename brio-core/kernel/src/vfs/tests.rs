use super::manager::{SessionError, SessionManager};
use crate::infrastructure::config::SandboxSettings;
use std::fs;
use tempfile::tempdir;

#[test]
fn test_rollback_basic() -> anyhow::Result<()> {
    let temp_dir = tempdir()?;
    let base_dir = temp_dir.path().join("base");
    fs::create_dir(&base_dir)?;

    fs::write(base_dir.join("file1.txt"), "original")?;

    let mut manager = SessionManager::new(&SandboxSettings::default())?;
    let session_id = manager.begin_session(&base_dir.to_string_lossy())?;
    let session_path = manager
        .session_path(&session_id)
        .ok_or(anyhow::anyhow!("session path not found"))?;

    fs::write(session_path.join("file1.txt"), "modified")?;

    manager.rollback_session(&session_id)?;

    assert_eq!(fs::read_to_string(base_dir.join("file1.txt"))?, "original");
    assert!(!session_path.exists());
    assert_eq!(manager.active_session_count(), 0);

    Ok(())
}

#[test]
fn test_rollback_nonexistent_session() -> anyhow::Result<()> {
    let temp_dir = tempdir()?;
    let base_dir = temp_dir.path().join("base");
    fs::create_dir(&base_dir)?;

    let mut manager = SessionManager::new(&SandboxSettings::default())?;

    let result = manager.rollback_session("invalid-session-id");

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(matches!(err, SessionError::SessionNotFound(_)));
    assert!(err.to_string().contains("invalid-session-id"));

    Ok(())
}

#[test]
fn test_rollback_cleanup() -> anyhow::Result<()> {
    let temp_dir = tempdir()?;
    let base_dir = temp_dir.path().join("base");
    fs::create_dir(&base_dir)?;

    let mut manager = SessionManager::new(&SandboxSettings::default())?;
    let session_id = manager.begin_session(&base_dir.to_string_lossy())?;
    let session_path = manager
        .session_path(&session_id)
        .ok_or(anyhow::anyhow!("session path not found"))?;

    fs::write(session_path.join("file1.txt"), "content1")?;
    fs::write(session_path.join("file2.txt"), "content2")?;
    fs::create_dir(session_path.join("subdir"))?;
    fs::write(session_path.join("subdir/file3.txt"), "content3")?;

    assert!(session_path.join("file1.txt").exists());
    assert!(session_path.join("file2.txt").exists());
    assert!(session_path.join("subdir/file3.txt").exists());

    manager.rollback_session(&session_id)?;

    assert!(!session_path.exists());
    assert_eq!(manager.active_session_count(), 0);

    Ok(())
}

#[test]
fn test_rollback_after_partial_changes() -> anyhow::Result<()> {
    let temp_dir = tempdir()?;
    let base_dir = temp_dir.path().join("base");
    fs::create_dir_all(&base_dir)?;
    fs::create_dir_all(base_dir.join("dir_a"))?;
    fs::create_dir_all(base_dir.join("dir_b"))?;
    fs::create_dir_all(base_dir.join("dir_c"))?;

    fs::write(base_dir.join("dir_a/file_a.txt"), "original a")?;
    fs::write(base_dir.join("dir_b/file_b.txt"), "original b")?;
    fs::write(base_dir.join("dir_c/file_c.txt"), "original c")?;

    let mut manager = SessionManager::new(&SandboxSettings::default())?;
    let session_id = manager.begin_session(&base_dir.to_string_lossy())?;
    let session_path = manager
        .session_path(&session_id)
        .ok_or(anyhow::anyhow!("session path not found"))?;

    fs::write(session_path.join("dir_a/file_a.txt"), "modified a")?;
    fs::write(session_path.join("file_new.txt"), "new file")?;
    fs::remove_file(session_path.join("dir_c/file_c.txt"))?;

    assert_eq!(
        fs::read_to_string(session_path.join("dir_a/file_a.txt"))?,
        "modified a"
    );
    assert!(session_path.join("file_new.txt").exists());
    assert!(!session_path.join("dir_c/file_c.txt").exists());

    manager.rollback_session(&session_id)?;

    assert_eq!(
        fs::read_to_string(base_dir.join("dir_a/file_a.txt"))?,
        "original a"
    );
    assert!(!base_dir.join("file_new.txt").exists());
    assert!(base_dir.join("dir_c/file_c.txt").exists());
    assert_eq!(
        fs::read_to_string(base_dir.join("dir_c/file_c.txt"))?,
        "original c"
    );
    assert_eq!(
        fs::read_to_string(base_dir.join("dir_b/file_b.txt"))?,
        "original b"
    );

    Ok(())
}

#[test]
fn test_session_lifecycle() -> anyhow::Result<()> {
    let temp_dir = std::env::temp_dir().join("brio_tests");
    let base_dir = temp_dir.join("base");

    if base_dir.exists() {
        fs::remove_dir_all(&base_dir)?;
    }
    fs::create_dir_all(&base_dir)?;

    fs::write(base_dir.join("file1.txt"), "original")?;
    fs::create_dir(base_dir.join("subdir"))?;
    fs::write(base_dir.join("subdir/file2.txt"), "sub")?;

    // 1. Begin Session
    let mut manager =
        SessionManager::new(&SandboxSettings::default()).map_err(|e| anyhow::anyhow!(e))?;
    let session_id = manager
        .begin_session(
            base_dir
                .to_str()
                .ok_or_else(|| anyhow::anyhow!("Invalid base dir"))?,
        )
        .map_err(|e| anyhow::anyhow!(e))?;

    let session_path = std::env::temp_dir().join("brio").join(&session_id);
    assert!(session_path.exists());
    assert_eq!(
        fs::read_to_string(session_path.join("file1.txt"))?,
        "original"
    );

    // 2. Modify Session
    fs::write(session_path.join("file1.txt"), "modified")?;
    fs::write(session_path.join("new.txt"), "created")?;
    fs::remove_file(session_path.join("subdir/file2.txt"))?;

    // 3. Commit Session
    manager
        .commit_session(&session_id)
        .map_err(|e| anyhow::anyhow!(e))?;

    // 4. Verify Base
    assert_eq!(fs::read_to_string(base_dir.join("file1.txt"))?, "modified");
    assert_eq!(fs::read_to_string(base_dir.join("new.txt"))?, "created");
    assert!(!base_dir.join("subdir/file2.txt").exists());

    let _ = fs::remove_dir_all(&base_dir);
    let _ = fs::remove_dir_all(&session_path);
    Ok(())
}

#[test]
fn test_begin_session_sandbox_violation() -> anyhow::Result<()> {
    // Setup a temp dir as our "allowed" root (though we won't put the target there)
    let temp_dir = tempdir()?;
    let allowed_path = temp_dir.path().join("allowed_project");
    fs::create_dir(&allowed_path)?;

    // Setup a target outside the allowed root
    let diff_path = temp_dir.path().join("forbidden_project");
    fs::create_dir(&diff_path)?;

    let sandbox = SandboxSettings {
        allowed_paths: vec![allowed_path.to_string_lossy().to_string()],
    };
    let mut manager = SessionManager::new(&sandbox).map_err(|e| anyhow::anyhow!(e))?;

    let result = manager.begin_session(&diff_path.to_string_lossy());

    assert!(result.is_err());
    let err = result
        .err()
        .ok_or_else(|| anyhow::anyhow!("Expected error"))?;
    assert!(matches!(err, SessionError::PolicyViolation(_)));
    Ok(())
}

#[test]
fn test_begin_session_sandbox_ok() -> anyhow::Result<()> {
    let temp_dir = tempdir()?;
    let allowed_path = temp_dir.path().join("allowed_project");
    fs::create_dir(&allowed_path)?;

    let sandbox = SandboxSettings {
        allowed_paths: vec![allowed_path.to_string_lossy().to_string()],
    };
    let mut manager = SessionManager::new(&sandbox).map_err(|e| anyhow::anyhow!(e))?;

    let result = manager.begin_session(&allowed_path.to_string_lossy());
    assert!(result.is_ok());
    Ok(())
}
