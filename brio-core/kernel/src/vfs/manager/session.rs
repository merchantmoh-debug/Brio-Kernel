//! Session management for isolated file system operations.
//!
//! This module manages temporary working directories for agents, providing
//! copy-on-write isolation through reflinks and atomic commit/rollback semantics.

use std::collections::HashMap;
use std::path::PathBuf;
use tracing::{info, instrument};
use uuid::Uuid;

use super::isolation::IsolationOps;
use super::types::{SessionError, SessionInfo};
use crate::infrastructure::config::SandboxSettings;
use crate::vfs::policy::SandboxPolicy;

/// Manages isolated file system sessions for agents.
pub struct SessionManager {
    sessions: HashMap<String, SessionInfo>,
    root_temp_dir: PathBuf,
    policy: SandboxPolicy,
    isolation: IsolationOps,
}

impl std::fmt::Debug for SessionManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionManager")
            .field("session_count", &self.sessions.len())
            .field("root_temp_dir", &self.root_temp_dir)
            .field("policy", &self.policy)
            .field("isolation", &self.isolation)
            .finish()
    }
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
            isolation: IsolationOps::new(),
        })
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

        let base_snapshot_hash = self
            .isolation
            .compute_hash(&canonical_base)
            .map_err(|e| SessionError::DiffFailed(e.clone()))?;

        self.isolation
            .copy_with_reflink(&canonical_base, &session_path)
            .map_err(|e| SessionError::CopyFailed(e.clone()))?;

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

        // Use isolation ops for conflict detection and commit
        self.isolation.commit_with_conflict_detection(
            &session_path,
            &base_path,
            &original_hash,
            session_id,
        )?;

        self.sessions.remove(session_id);
        self.cleanup_session_dir(session_id)?;

        info!(
            "Session {} committed and cleaned up successfully",
            session_id
        );
        Ok(())
    }

    /// Rolls back a session, discarding all changes.
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

    /// Returns a list of all active sessions with their metadata.
    #[must_use]
    pub fn list_sessions(&self) -> Vec<(String, crate::vfs::manager::types::SessionInfo)> {
        self.sessions
            .iter()
            .map(|(id, info)| (id.clone(), info.clone()))
            .collect()
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
        self.isolation
            .cleanup_orphaned(&self.root_temp_dir, &self.sessions)
    }

    /// Cleans up the temporary session directory.
    /// This is called automatically after commit or rollback.
    fn cleanup_session_dir(&self, session_id: &str) -> Result<(), SessionError> {
        self.isolation
            .cleanup_session(&self.root_temp_dir, session_id)
    }
}

impl Default for SessionManager {
    fn default() -> Self {
        Self {
            sessions: HashMap::new(),
            root_temp_dir: std::env::temp_dir().join("brio"),
            policy: SandboxPolicy::new_empty(),
            isolation: IsolationOps::new(),
        }
    }
}
