//! Session manager for isolated file system operations.
//!
//! This module manages temporary working directories for agents, providing
//! copy-on-write isolation through reflinks and atomic commit/rollback semantics.

use super::{diff, hashing, policy::SandboxPolicy, reflink};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use thiserror::Error;
use tracing::{debug, info, instrument, warn};
use uuid::Uuid;

use crate::infrastructure::config::SandboxSettings;

/// Errors that can occur during VFS session operations.
#[derive(Debug, Error)]
pub enum SessionError {
    /// The session ID was not found in the active sessions.
    #[error("Session not found: {0}")]
    SessionNotFound(String),
    /// The base path provided for the session is invalid or does not exist.
    #[error("Invalid base path '{path}': {source}")]
    InvalidBasePath {
        /// Path that was invalid.
        path: String,
        /// Source error.
        #[source]
        source: std::io::Error,
    },
    /// The base path does not exist.
    #[error("Base path does not exist: {0}")]
    BasePathNotFound(String),
    /// A sandbox policy violation occurred.
    #[error("Sandbox policy violation: {0}")]
    PolicyViolation(String),
    /// Failed to create a session copy using reflink.
    #[error("Failed to create session copy: {0}")]
    CopyFailed(String),
    /// Failed to compute or apply diff between session and base.
    #[error("Diff operation failed: {0}")]
    DiffFailed(String),
    /// A conflict was detected between session and base directory.
    #[error(
        "Conflict: base directory '{path}' has been modified since session started. Original hash: {original_hash}, Current hash: {current_hash}"
    )]
    Conflict {
        /// Path where conflict was detected.
        path: PathBuf,
        /// Original hash of the base directory.
        original_hash: String,
        /// Current hash of the base directory.
        current_hash: String,
    },
    /// The session directory was lost or deleted.
    #[error("Session directory lost: {0}")]
    SessionDirectoryLost(PathBuf),
    /// Failed to cleanup session directory.
    #[error("Failed to cleanup session directory {path}: {source}")]
    CleanupFailed {
        /// Path of the session directory.
        path: PathBuf,
        /// Source error.
        #[source]
        source: std::io::Error,
    },
    /// Failed to read directory contents.
    #[error("Failed to read directory: {0}")]
    ReadDirectoryFailed(String),
}

/// Represents a session with its base path and snapshot hash
struct SessionInfo {
    base_path: PathBuf,
    /// Hash of the base directory at session start (for conflict detection)
    base_snapshot_hash: String,
}

/// Manages isolated file system sessions for agents.
pub struct SessionManager {
    sessions: HashMap<String, SessionInfo>,
    root_temp_dir: PathBuf,
    policy: SandboxPolicy,
}

impl SessionManager {
    /// Creates a new session manager with the specified sandbox settings.
    ///
    /// # Errors
    ///
    /// Returns an error if the sandbox policy cannot be initialized.
    pub fn new(sandbox: &SandboxSettings) -> Result<Self, SessionError> {
        // Use standard temp dir or default to /tmp/brio
        let temp = std::env::temp_dir().join("brio");
        Ok(Self {
            sessions: HashMap::new(),
            root_temp_dir: temp,
            policy: SandboxPolicy::new(sandbox)
                .map_err(|e| SessionError::PolicyViolation(e.to_string()))?,
        })
    }

    /// Cleans up the temporary session directory.
    /// This is called automatically after commit or rollback.
    fn cleanup_session_dir(&self, session_id: &str) -> Result<(), SessionError> {
        let session_path = self.root_temp_dir.join(session_id);
        if session_path.exists() {
            fs::remove_dir_all(&session_path).map_err(|e| SessionError::CleanupFailed {
                path: session_path.clone(),
                source: e,
            })?;
            debug!("Cleaned up session directory: {:?}", session_path);
        }
        Ok(())
    }

    /// Returns the path to the session's working directory.
    /// Useful for agents that need to know where to make changes.
    #[must_use]
    pub fn session_path(&self, session_id: &str) -> Option<PathBuf> {
        if self.sessions.contains_key(session_id) {
            Some(self.root_temp_dir.join(session_id))
        } else {
            None
        }
    }

    /// Creates a new session by copying (reflink) the base directory.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The base path does not exist or is invalid
    /// - The path violates sandbox policy
    /// - The session copy cannot be created
    #[instrument(skip(self))]
    pub fn begin_session(&mut self, base_path: &str) -> Result<String, SessionError> {
        let canonical_base =
            dunce::canonicalize(base_path).map_err(|e| SessionError::InvalidBasePath {
                path: base_path.to_string(),
                source: e,
            })?;

        if !canonical_base.exists() {
            return Err(SessionError::BasePathNotFound(base_path.to_string()));
        }

        self.policy
            .validate_path(&canonical_base)
            .map_err(|e| SessionError::PolicyViolation(e.to_string()))?;

        let session_id = Uuid::new_v4().to_string();
        let session_path = self.root_temp_dir.join(&session_id);

        info!(
            "Starting session {} for base {:?}",
            session_id, canonical_base
        );

        let base_snapshot_hash = hashing::compute_directory_hash(&canonical_base)
            .map_err(|e| SessionError::DiffFailed(e.clone()))?;

        reflink::copy_dir_reflink(&canonical_base, &session_path)
            .map_err(|e| SessionError::CopyFailed(e.to_string()))?;

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
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The session is not found
    /// - The session directory has been lost
    /// - The base directory was modified since session start (conflict)
    /// - Changes cannot be computed or applied
    #[instrument(skip(self))]
    pub fn commit_session(&mut self, session_id: &str) -> Result<(), SessionError> {
        let session_info = self
            .sessions
            .get(session_id)
            .ok_or_else(|| SessionError::SessionNotFound(session_id.to_string()))?;

        let base_path = session_info.base_path.clone();
        let original_hash = session_info.base_snapshot_hash.clone();
        let session_path = self.root_temp_dir.join(session_id);

        if !session_path.exists() {
            self.sessions.remove(session_id);
            return Err(SessionError::SessionDirectoryLost(session_path.clone()));
        }

        // Conflict detection: re-hash base and compare
        let current_hash = hashing::compute_directory_hash(&base_path)
            .map_err(|e| SessionError::DiffFailed(e.clone()))?;
        if current_hash != original_hash {
            warn!(
                "Conflict detected for session {}: base directory has been modified",
                session_id
            );
            return Err(SessionError::Conflict {
                path: base_path,
                original_hash,
                current_hash,
            });
        }

        info!("Committing session {} to {:?}", session_id, base_path);

        let changes = diff::compute_diff(&session_path, &base_path)
            .map_err(|e| SessionError::DiffFailed(e.to_string()))?;

        if changes.is_empty() {
            info!("No changes to commit for session {}", session_id);
            self.sessions.remove(session_id);
            self.cleanup_session_dir(session_id)?;
            return Ok(());
        }

        diff::apply_changes(&session_path, &base_path, &changes)
            .map_err(|e| SessionError::DiffFailed(e.to_string()))?;

        self.sessions.remove(session_id);
        self.cleanup_session_dir(session_id)?;

        info!(
            "Session {} committed and cleaned up successfully",
            session_id
        );
        Ok(())
    }

    /// Rolls back a session, discarding all changes without applying them.
    /// This removes the session from tracking and cleans up the temp directory.
    ///
    /// # Errors
    ///
    /// Returns an error if the session is not found or if cleanup fails.
    #[instrument(skip(self))]
    pub fn rollback_session(&mut self, session_id: &str) -> Result<(), SessionError> {
        if !self.sessions.contains_key(session_id) {
            return Err(SessionError::SessionNotFound(session_id.to_string()));
        }

        info!("Rolling back session {}", session_id);

        self.sessions.remove(session_id);
        self.cleanup_session_dir(session_id)?;

        info!("Session {} rolled back and cleaned up", session_id);
        Ok(())
    }

    /// Returns the number of active sessions.
    #[must_use]
    pub fn active_session_count(&self) -> usize {
        self.sessions.len()
    }

    /// Cleans up all orphaned session directories that are not being tracked.
    /// This can be called on startup to recover from crashes.
    ///
    /// # Errors
    ///
    /// Returns an error if the temp directory cannot be read or if orphaned
    /// directories cannot be removed.
    #[instrument(skip(self))]
    pub fn cleanup_orphaned_sessions(&self) -> Result<usize, SessionError> {
        let mut cleaned = 0;

        if !self.root_temp_dir.exists() {
            return Ok(0);
        }

        let entries = fs::read_dir(&self.root_temp_dir)
            .map_err(|e| SessionError::ReadDirectoryFailed(e.to_string()))?;

        for entry in entries {
            let entry = entry.map_err(|e| SessionError::ReadDirectoryFailed(e.to_string()))?;
            let path = entry.path();

            if path.is_dir() {
                let dir_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

                if !self.sessions.contains_key(dir_name) {
                    info!("Cleaning up orphaned session directory: {:?}", path);
                    fs::remove_dir_all(&path).map_err(|e| SessionError::CleanupFailed {
                        path: path.clone(),
                        source: e,
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
