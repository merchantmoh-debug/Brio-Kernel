use super::{diff, hashing, policy::SandboxPolicy, reflink};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use tracing::{debug, info, instrument, warn};
use uuid::Uuid;

/// Represents a session with its base path and snapshot hash
struct SessionInfo {
    base_path: PathBuf,
    /// Hash of the base directory at session start (for conflict detection)
    base_snapshot_hash: String,
}

use crate::infrastructure::config::SandboxSettings;

pub struct SessionManager {
    // Map SessionID -> SessionInfo
    sessions: HashMap<String, SessionInfo>,
    root_temp_dir: PathBuf,
    policy: SandboxPolicy,
}

impl SessionManager {
    pub fn new(sandbox: SandboxSettings) -> Result<Self, String> {
        // Use standard temp dir or default to /tmp/brio
        let temp = std::env::temp_dir().join("brio");
        Ok(Self {
            sessions: HashMap::new(),
            root_temp_dir: temp,
            policy: SandboxPolicy::new(&sandbox).map_err(|e| e.to_string())?,
        })
    }

    /// Cleans up the temporary session directory.
    /// This is called automatically after commit or rollback.
    fn cleanup_session_dir(&self, session_id: &str) -> Result<(), String> {
        let session_path = self.root_temp_dir.join(session_id);
        if session_path.exists() {
            fs::remove_dir_all(&session_path).map_err(|e| {
                format!(
                    "Failed to cleanup session directory {:?}: {}",
                    session_path, e
                )
            })?;
            debug!("Cleaned up session directory: {:?}", session_path);
        }
        Ok(())
    }

    /// Returns the path to the session's working directory.
    /// Useful for agents that need to know where to make changes.
    pub fn get_session_path(&self, session_id: &str) -> Option<PathBuf> {
        if self.sessions.contains_key(session_id) {
            Some(self.root_temp_dir.join(session_id))
        } else {
            None
        }
    }

    /// Creates a new session by copying (reflink) the base directory.
    #[instrument(skip(self))]
    pub fn begin_session(&mut self, base_path: String) -> Result<String, String> {
        // Security: Path Sandboxing (Delegated to Policy)
        let canonical_base = dunce::canonicalize(&base_path)
            .map_err(|e| format!("Invalid base path '{}': {}", base_path, e))?;

        // 1. Check if path exists
        if !canonical_base.exists() {
            return Err(format!("Base path does not exist: {}", base_path));
        }

        // 2. Enforce Sandbox Limits
        self.policy
            .validate_path(&canonical_base)
            .map_err(|e| e.to_string())?;

        let session_id = Uuid::new_v4().to_string();
        let session_path = self.root_temp_dir.join(&session_id);

        info!(
            "Starting session {} for base {:?}",
            session_id, canonical_base
        );

        // Compute snapshot hash before copying (Delegated to hashing)
        let base_snapshot_hash = hashing::compute_directory_hash(&canonical_base)?;

        // Perform Reflink Copy
        reflink::copy_dir_reflink(&canonical_base, &session_path)
            .map_err(|e| format!("Failed to create session copy: {}", e))?;

        // Store session mapping with snapshot
        self.sessions.insert(
            session_id.clone(),
            SessionInfo {
                base_path: canonical_base,
                base_snapshot_hash,
            },
        );

        Ok(session_id)
    }

    /// Commits changes from the session back to the base directory.
    /// Returns an error if the base directory has been modified since session start.
    /// Automatically cleans up the session directory after successful commit.
    #[instrument(skip(self))]
    pub fn commit_session(&mut self, session_id: String) -> Result<(), String> {
        let session_info = self
            .sessions
            .get(&session_id)
            .ok_or_else(|| format!("Session not found: {}", session_id))?;

        let base_path = session_info.base_path.clone();
        let original_hash = session_info.base_snapshot_hash.clone();
        let session_path = self.root_temp_dir.join(&session_id);

        if !session_path.exists() {
            self.sessions.remove(&session_id);
            return Err(format!("Session directory lost: {:?}", session_path));
        }

        // Conflict detection: re-hash base and compare
        let current_hash = hashing::compute_directory_hash(&base_path)?;
        if current_hash != original_hash {
            warn!(
                "Conflict detected for session {}: base directory has been modified",
                session_id
            );
            return Err(format!(
                "Conflict: base directory '{}' has been modified since session started. \
                 Original hash: {}, Current hash: {}",
                base_path.display(),
                original_hash,
                current_hash
            ));
        }

        info!("Committing session {} to {:?}", session_id, base_path);

        // 1. Compute Diff
        let changes = diff::compute_diff(&session_path, &base_path)
            .map_err(|e| format!("Failed to compute diff: {}", e))?;

        if changes.is_empty() {
            info!("No changes to commit for session {}", session_id);
            // Still cleanup even if no changes
            self.sessions.remove(&session_id);
            self.cleanup_session_dir(&session_id)?;
            return Ok(());
        }

        // 2. Apply Changes
        diff::apply_changes(&session_path, &base_path, &changes)
            .map_err(|e| format!("Failed to apply changes: {}", e))?;

        // 3. Cleanup session from map and filesystem
        self.sessions.remove(&session_id);
        self.cleanup_session_dir(&session_id)?;

        info!(
            "Session {} committed and cleaned up successfully",
            session_id
        );
        Ok(())
    }

    /// Rolls back a session, discarding all changes without applying them.
    /// This removes the session from tracking and cleans up the temp directory.
    #[instrument(skip(self))]
    pub fn rollback_session(&mut self, session_id: String) -> Result<(), String> {
        if !self.sessions.contains_key(&session_id) {
            return Err(format!("Session not found: {}", session_id));
        }

        info!("Rolling back session {}", session_id);

        // Remove from tracking
        self.sessions.remove(&session_id);

        // Cleanup temp directory
        self.cleanup_session_dir(&session_id)?;

        info!("Session {} rolled back and cleaned up", session_id);
        Ok(())
    }

    /// Returns the number of active sessions.
    pub fn active_session_count(&self) -> usize {
        self.sessions.len()
    }

    /// Cleans up all orphaned session directories that are not being tracked.
    /// This can be called on startup to recover from crashes.
    #[instrument(skip(self))]
    pub fn cleanup_orphaned_sessions(&self) -> Result<usize, String> {
        let mut cleaned = 0;

        if !self.root_temp_dir.exists() {
            return Ok(0);
        }

        let entries = fs::read_dir(&self.root_temp_dir)
            .map_err(|e| format!("Failed to read temp directory: {}", e))?;

        for entry in entries {
            let entry = entry.map_err(|e| format!("Failed to read directory entry: {}", e))?;
            let path = entry.path();

            if path.is_dir() {
                let dir_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

                // If this directory is not tracked, it's orphaned
                if !self.sessions.contains_key(dir_name) {
                    info!("Cleaning up orphaned session directory: {:?}", path);
                    fs::remove_dir_all(&path).map_err(|e| {
                        format!("Failed to remove orphaned directory {:?}: {}", path, e)
                    })?;
                    cleaned += 1;
                }
            }
        }

        if cleaned > 0 {
            info!("Cleaned up {} orphaned session directories", cleaned);
        }

        Ok(cleaned)
    }
}

impl Default for SessionManager {
    fn default() -> Self {
        Self {
            sessions: HashMap::new(),
            root_temp_dir: std::env::temp_dir().join("brio"),
            policy: SandboxPolicy::new_empty(),
        }
    }
}
