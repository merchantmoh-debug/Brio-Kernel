//! Types for VFS session management.
//!
//! This module provides error types and data structures for session management.

use std::path::PathBuf;
use thiserror::Error;

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

/// Represents a session with its base path and snapshot hash.
#[derive(Debug, Clone)]
pub struct SessionInfo {
    /// The base path of the session.
    pub(crate) base_path: PathBuf,
    /// Hash of the base directory at session start (for conflict detection).
    pub(crate) base_snapshot_hash: String,
}

impl SessionInfo {
    /// Get the base path.
    #[must_use]
    pub fn base_path(&self) -> &PathBuf {
        &self.base_path
    }

    /// Get the base snapshot hash.
    #[must_use]
    pub fn base_snapshot_hash(&self) -> &str {
        &self.base_snapshot_hash
    }
}
