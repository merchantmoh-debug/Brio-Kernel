//! Copy-on-write isolation for VFS sessions.
//!
//! This module provides the isolation mechanisms for VFS sessions,
//! including reflink copying, conflict detection, and atomic commits.

use std::collections::HashMap;
use tracing::{debug, info, warn};

use crate::vfs::manager::SessionError;
use crate::vfs::manager::types::SessionInfo;
use crate::vfs::{diff, hashing, reflink};

/// Operations for copy-on-write isolation.
#[derive(Debug, Clone)]
pub struct IsolationOps;

impl IsolationOps {
    /// Create a new isolation operations instance.
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// Compute directory hash for conflict detection.
    ///
    /// # Errors
    ///
    /// Returns an error if the hash computation fails.
    pub fn compute_hash(&self, path: &std::path::Path) -> Result<String, String> {
        hashing::compute_directory_hash(path)
    }

    /// Copy directory using reflink (copy-on-write).
    ///
    /// # Errors
    ///
    /// Returns an error if the reflink copy operation fails.
    pub fn copy_with_reflink(
        &self,
        source: &std::path::Path,
        destination: &std::path::Path,
    ) -> Result<(), String> {
        match reflink::copy_dir_reflink(source, destination) {
            Ok(()) => Ok(()),
            Err(e) => Err(e.to_string()),
        }
    }

    /// Commit session with conflict detection.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Conflict is detected (base directory modified)
    /// - Diff computation fails
    /// - Change application fails
    pub fn commit_with_conflict_detection(
        &self,
        session_path: &std::path::Path,
        base_path: &std::path::Path,
        original_hash: &str,
        session_id: &str,
    ) -> Result<(), SessionError> {
        // Conflict detection: re-hash base and compare
        let current_hash = self
            .compute_hash(base_path)
            .map_err(|e| SessionError::DiffFailed(e.clone()))?;

        if current_hash != original_hash {
            warn!(
                "Conflict detected for session {}: base directory has been modified",
                session_id
            );
            return Err(SessionError::Conflict {
                path: base_path.to_path_buf(),
                original_hash: original_hash.to_string(),
                current_hash,
            });
        }

        info!("Committing session {} to {:?}", session_id, base_path);

        let changes = diff::compute_diff(session_path, base_path)
            .map_err(|e| SessionError::DiffFailed(e.to_string()))?;

        if changes.is_empty() {
            info!("No changes to commit for session {}", session_id);
            return Ok(());
        }

        diff::apply_changes(session_path, base_path, &changes)
            .map_err(|e| SessionError::DiffFailed(e.to_string()))?;

        info!(
            "Session {} committed successfully with {} changes",
            session_id,
            changes.len()
        );
        Ok(())
    }

    /// Clean up a specific session directory.
    ///
    /// # Errors
    ///
    /// Returns an error if the directory cannot be removed.
    pub fn cleanup_session(
        &self,
        root_temp_dir: &std::path::Path,
        session_id: &str,
    ) -> Result<(), SessionError> {
        let session_path = root_temp_dir.join(session_id);
        if session_path.exists() {
            std::fs::remove_dir_all(&session_path).map_err(|e| SessionError::CleanupFailed {
                path: session_path.clone(),
                source: e,
            })?;
            debug!("Cleaned up session directory: {:?}", session_path);
        }
        Ok(())
    }

    /// Clean up orphaned session directories.
    ///
    /// # Errors
    ///
    /// Returns an error if the temp directory cannot be read or if orphaned
    /// directories cannot be removed.
    pub fn cleanup_orphaned(
        &self,
        root_temp_dir: &std::path::Path,
        sessions: &HashMap<String, SessionInfo>,
    ) -> Result<usize, SessionError> {
        let mut cleaned = 0;

        if !root_temp_dir.exists() {
            return Ok(0);
        }

        let entries = std::fs::read_dir(root_temp_dir)
            .map_err(|e| SessionError::ReadDirectoryFailed(e.to_string()))?;

        for entry in entries {
            let entry = entry.map_err(|e| SessionError::ReadDirectoryFailed(e.to_string()))?;
            let path = entry.path();

            if path.is_dir() {
                let dir_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

                if !sessions.contains_key(dir_name) {
                    info!("Cleaning up orphaned session directory: {:?}", path);
                    std::fs::remove_dir_all(&path).map_err(|e| SessionError::CleanupFailed {
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

impl Default for IsolationOps {
    fn default() -> Self {
        Self::new()
    }
}
