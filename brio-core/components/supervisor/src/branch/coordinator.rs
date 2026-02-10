//! Branch Coordinator - High-level coordination of branch operations.
//!
//! This module provides the core coordination logic, error types, and
//! the `BranchManager` struct that ties together all branch functionality.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use crate::branch::{Branch, BranchSource};
use crate::domain::{BranchId, BranchStatus, BranchValidationError};
use crate::merge::{MergeError, MergeId, MergeStrategyRegistry};
use crate::repository::{BranchRepository, BranchRepositoryError};

/// Unique identifier for a merge request.
pub type MergeRequestId = MergeId;

/// Errors that can occur during VFS session operations.
#[derive(Debug, Clone)]
pub enum SessionError {
    /// The session ID was not found in the active sessions.
    SessionNotFound(String),
    /// The base path provided for the session is invalid or does not exist.
    InvalidBasePath {
        /// Path that was invalid.
        path: String,
        /// Source error message.
        source: String,
    },
    /// The base path does not exist.
    BasePathNotFound(String),
    /// A sandbox policy violation occurred.
    PolicyViolation(String),
    /// Failed to create a session copy using reflink.
    CopyFailed(String),
    /// Failed to compute or apply diff between session and base.
    DiffFailed(String),
    /// A conflict was detected between session and base directory.
    Conflict {
        /// Path where conflict was detected.
        path: PathBuf,
        /// Original hash of the base directory.
        original_hash: String,
        /// Current hash of the base directory.
        current_hash: String,
    },
    /// The session directory was lost or deleted.
    SessionDirectoryLost(PathBuf),
    /// Failed to cleanup session directory.
    CleanupFailed {
        /// Path of the session directory.
        path: PathBuf,
        /// Source error message.
        source: String,
    },
    /// Failed to read directory contents.
    ReadDirectoryFailed(String),
}

impl core::fmt::Display for SessionError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::SessionNotFound(id) => write!(f, "Session not found: {id}"),
            Self::InvalidBasePath { path, source } => {
                write!(f, "Invalid base path '{path}': {source}")
            }
            Self::BasePathNotFound(path) => write!(f, "Base path not found: {path}"),
            Self::PolicyViolation(msg) => write!(f, "Policy violation: {msg}"),
            Self::CopyFailed(msg) => write!(f, "Copy failed: {msg}"),
            Self::DiffFailed(msg) => write!(f, "Diff failed: {msg}"),
            Self::Conflict {
                path,
                original_hash,
                current_hash,
            } => {
                write!(
                    f,
                    "Conflict at {path:?}: hash changed from {original_hash} to {current_hash}"
                )
            }
            Self::SessionDirectoryLost(path) => write!(f, "Session directory lost: {path:?}"),
            Self::CleanupFailed { path, source } => {
                write!(f, "Cleanup failed at {path:?}: {source}")
            }
            Self::ReadDirectoryFailed(msg) => write!(f, "Read directory failed: {msg}"),
        }
    }
}

impl std::error::Error for SessionError {}

/// Error type for branch operations.
#[derive(Debug)]
pub enum BranchError {
    /// Repository operation failed.
    Repository(BranchRepositoryError),
    /// Session operation failed.
    Session(SessionError),
    /// Merge operation failed.
    Merge(MergeError),
    /// Branch not found.
    BranchNotFound(BranchId),
    /// Maximum number of branches exceeded.
    MaxBranchesExceeded {
        /// Current number of branches.
        current: usize,
        /// Maximum allowed branches.
        limit: usize,
    },
    /// Invalid status transition.
    InvalidStatusTransition {
        /// Current status.
        from: BranchStatus,
        /// Target status.
        to: BranchStatus,
    },
    /// Merge not approved.
    MergeNotApproved(MergeRequestId),
    /// Invalid merge strategy.
    InvalidStrategy(String),
    /// Validation error.
    Validation(BranchValidationError),
    /// Merge request not found.
    MergeRequestNotFound(MergeRequestId),
    /// Branch is not in the correct state for this operation.
    InvalidBranchState {
        /// The branch ID.
        branch_id: BranchId,
        /// Expected state.
        expected: String,
        /// Actual state.
        actual: String,
    },
}

impl core::fmt::Display for BranchError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Repository(e) => write!(f, "Repository error: {e}"),
            Self::Session(e) => write!(f, "Session error: {e}"),
            Self::Merge(e) => write!(f, "Merge error: {e}"),
            Self::BranchNotFound(id) => write!(f, "Branch not found: {id}"),
            Self::MaxBranchesExceeded { current, limit } => {
                write!(f, "Max branches exceeded: {current}/{limit}")
            }
            Self::InvalidStatusTransition { from, to } => {
                write!(f, "Invalid status transition: {from:?} -> {to:?}")
            }
            Self::MergeNotApproved(id) => write!(f, "Merge not approved: {id}"),
            Self::InvalidStrategy(s) => write!(f, "Invalid merge strategy: {s}"),
            Self::Validation(e) => write!(f, "Validation error: {e}"),
            Self::MergeRequestNotFound(id) => write!(f, "Merge request not found: {id}"),
            Self::InvalidBranchState {
                branch_id,
                expected,
                actual,
            } => {
                write!(
                    f,
                    "Invalid branch state for {branch_id}: expected {expected}, got {actual}"
                )
            }
        }
    }
}

impl std::error::Error for BranchError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Repository(e) => Some(e),
            Self::Session(e) => Some(e),
            Self::Merge(e) => Some(e),
            Self::Validation(e) => Some(e),
            _ => None,
        }
    }
}

impl From<BranchRepositoryError> for BranchError {
    fn from(e: BranchRepositoryError) -> Self {
        Self::Repository(e)
    }
}

impl From<SessionError> for BranchError {
    fn from(e: SessionError) -> Self {
        Self::Session(e)
    }
}

impl From<MergeError> for BranchError {
    fn from(e: MergeError) -> Self {
        Self::Merge(e)
    }
}

impl From<BranchValidationError> for BranchError {
    fn from(e: BranchValidationError) -> Self {
        Self::Validation(e)
    }
}

/// Tree structure representing a branch and its children.
#[derive(Debug, Clone)]
pub struct BranchTree {
    /// The branch at this node.
    pub branch: Branch,
    /// Child branches.
    pub children: Vec<BranchTree>,
}

impl BranchTree {
    /// Creates a new branch tree node.
    #[must_use]
    pub fn new(branch: Branch) -> Self {
        Self {
            branch,
            children: Vec::new(),
        }
    }

    /// Adds a child branch tree to this node.
    pub fn add_child(&mut self, child: BranchTree) {
        self.children.push(child);
    }

    /// Returns the total number of nodes in the tree.
    #[must_use]
    pub fn total_nodes(&self) -> usize {
        1 + self
            .children
            .iter()
            .map(BranchTree::total_nodes)
            .sum::<usize>()
    }
}

/// Trait for session management operations.
///
/// This trait abstracts the VFS session management, enabling:
/// - Unit testing with mock implementations
/// - Different backends (e.g., kernel VFS, mock for testing)
pub trait SessionManager: Send + Sync {
    /// Creates a new session by copying the base directory.
    ///
    /// # Errors
    /// Returns `SessionError` if the base path is invalid or session creation fails.
    fn begin_session(&mut self, base_path: &str) -> Result<String, SessionError>;

    /// Commits changes from the session back to the base directory.
    ///
    /// # Errors
    /// Returns `SessionError` if the session is not found or commit fails.
    fn commit_session(&mut self, session_id: &str) -> Result<(), SessionError>;

    /// Rolls back a session, discarding all changes.
    ///
    /// # Errors
    /// Returns `SessionError` if the session is not found or rollback fails.
    fn rollback_session(&mut self, session_id: &str) -> Result<(), SessionError>;

    /// Returns the path to the session's working directory.
    fn session_path(&self, session_id: &str) -> Option<PathBuf>;

    /// Returns the number of active sessions.
    fn active_session_count(&self) -> usize;
}

/// Manages branch lifecycle and coordinates with persistence and VFS.
pub struct BranchManager {
    pub(super) session_manager: Arc<Mutex<dyn SessionManager>>,
    pub(super) repository: Arc<dyn BranchRepository>,
    pub(super) merge_registry: MergeStrategyRegistry,
    pub(super) max_branches: usize,
}

impl std::fmt::Debug for BranchManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BranchManager")
            .field("max_branches", &self.max_branches)
            .field("merge_registry", &self.merge_registry)
            .finish_non_exhaustive()
    }
}

impl BranchManager {
    /// Creates a new `BranchManager` with the specified dependencies.
    #[must_use]
    pub fn new(
        session_manager: Arc<Mutex<dyn SessionManager>>,
        repository: Arc<dyn BranchRepository>,
        merge_registry: MergeStrategyRegistry,
    ) -> Self {
        Self {
            session_manager,
            repository,
            merge_registry,
            max_branches: 8,
        }
    }

    /// Creates a new `BranchManager` with a custom branch limit.
    #[must_use]
    pub fn with_max_branches(
        session_manager: Arc<Mutex<dyn SessionManager>>,
        repository: Arc<dyn BranchRepository>,
        merge_registry: MergeStrategyRegistry,
        max_branches: usize,
    ) -> Self {
        Self {
            session_manager,
            repository,
            merge_registry,
            max_branches,
        }
    }

    /// Acquires a lock on the session manager and returns the guard.
    ///
    /// This helper eliminates the repeated mutex lock pattern throughout the codebase.
    pub(super) fn lock_session_manager(
        &self,
    ) -> Result<std::sync::MutexGuard<'_, dyn SessionManager + 'static>, BranchError> {
        self.session_manager.lock().map_err(|_| {
            BranchError::Session(SessionError::SessionNotFound(
                "Failed to lock session manager".to_string(),
            ))
        })
    }

    /// Checks if we can create more branches.
    pub(super) fn check_branch_limit(&self) -> Result<(), BranchError> {
        let active_count = self.repository.list_active_branches()?.len();
        if active_count >= self.max_branches {
            return Err(BranchError::MaxBranchesExceeded {
                current: active_count,
                limit: self.max_branches,
            });
        }
        Ok(())
    }

    /// Generates the next branch ID.
    pub(super) fn next_branch_id(&mut self) -> BranchId {
        BranchId::new()
    }

    /// Gets the base path from a branch source.
    pub(super) fn get_base_path_from_source(
        &self,
        source: &BranchSource,
    ) -> Result<PathBuf, BranchError> {
        match source {
            BranchSource::Base(path) => Ok(path.clone()),
            BranchSource::Branch(parent_id) => {
                let parent = self
                    .repository
                    .get_branch(*parent_id)?
                    .ok_or(BranchError::BranchNotFound(*parent_id))?;
                // Get the session path for the parent branch
                let session_manager = self.lock_session_manager()?;
                session_manager
                    .session_path(parent.session_id())
                    .ok_or_else(|| {
                        BranchError::Session(SessionError::SessionNotFound(
                            parent.session_id().to_string(),
                        ))
                    })
            }
            BranchSource::Snapshot(_) => {
                // TODO: Implement snapshot-based branching
                Err(BranchError::Validation(
                    BranchValidationError::InvalidExecutionStrategy {
                        reason: "Snapshot branching not implemented".to_string(),
                    },
                ))
            }
        }
    }

    /// Gets a branch by its ID.
    ///
    /// # Errors
    ///
    /// Returns `BranchError::Repository` if the query fails.
    pub fn get_branch(&self, id: BranchId) -> Result<Option<Branch>, BranchError> {
        self.repository
            .get_branch(id)
            .map_err(BranchError::from)?
            .map(|record| Branch::try_from_record(&record))
            .transpose()
            .map_err(BranchError::Validation)
    }

    /// Lists all active branches.
    ///
    /// # Errors
    ///
    /// Returns `BranchError::Repository` if the query fails.
    pub fn list_active_branches(&self) -> Result<Vec<Branch>, BranchError> {
        self.repository
            .list_active_branches()
            .map_err(BranchError::from)?
            .iter()
            .map(Branch::try_from_record)
            .collect::<Result<Vec<_>, _>>()
            .map_err(BranchError::Validation)
    }

    /// Lists child branches of a parent branch.
    ///
    /// # Errors
    ///
    /// Returns `BranchError::Repository` if the query fails.
    pub fn list_child_branches(&self, parent_id: BranchId) -> Result<Vec<Branch>, BranchError> {
        self.repository
            .list_branches_by_parent(parent_id)
            .map_err(BranchError::from)?
            .iter()
            .map(Branch::try_from_record)
            .collect::<Result<Vec<_>, _>>()
            .map_err(BranchError::Validation)
    }

    /// Gets the branch tree starting from a root branch.
    ///
    /// # Errors
    ///
    /// Returns `BranchError::BranchNotFound` if the root branch doesn't exist.
    pub fn get_branch_tree(&self, root_id: BranchId) -> Result<BranchTree, BranchError> {
        // Get the root branch record
        let root_record = self
            .repository
            .get_branch(root_id)?
            .ok_or(BranchError::BranchNotFound(root_id))?;

        // Convert to domain entity
        let root = Branch::try_from_record(&root_record).map_err(BranchError::Validation)?;

        // Recursively build the tree
        self.build_branch_tree(root)
    }

    /// Recursively builds a branch tree.
    fn build_branch_tree(&self, branch: Branch) -> Result<BranchTree, BranchError> {
        let mut tree = BranchTree::new(branch.clone());

        // Get child branch records
        let child_records = self.repository.list_branches_by_parent(branch.id())?;

        for child_record in child_records {
            // Convert record to domain entity
            let child = Branch::try_from_record(&child_record).map_err(BranchError::Validation)?;
            let child_tree = self.build_branch_tree(child)?;
            tree.add_child(child_tree);
        }

        Ok(tree)
    }

    /// Returns the maximum number of branches allowed.
    #[must_use]
    pub fn max_branches(&self) -> usize {
        self.max_branches
    }

    /// Returns the current number of active branches.
    ///
    /// # Errors
    ///
    /// Returns `BranchError::Repository` if the query fails.
    pub fn active_branch_count(&self) -> Result<usize, BranchError> {
        Ok(self.repository.list_active_branches()?.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn branch_error_display() {
        let id = BranchId::from_uuid(uuid::Uuid::from_u128(1));
        let err = BranchError::BranchNotFound(id);
        assert!(err.to_string().contains("not found"));

        let err = BranchError::MaxBranchesExceeded {
            current: 10,
            limit: 8,
        };
        assert!(err.to_string().contains("10/8"));

        let err = BranchError::InvalidStrategy("bad-strategy".to_string());
        assert!(err.to_string().contains("bad-strategy"));
    }

    #[test]
    fn branch_error_from_repository_error() {
        let id = BranchId::from_uuid(uuid::Uuid::from_u128(1));
        let repo_err = BranchRepositoryError::BranchNotFound(id);
        let branch_err: BranchError = repo_err.into();
        assert!(matches!(branch_err, BranchError::Repository(_)));
    }

    #[test]
    fn branch_error_from_session_error() {
        let session_err = SessionError::SessionNotFound("test".to_string());
        let branch_err: BranchError = session_err.into();
        assert!(matches!(branch_err, BranchError::Session(_)));
    }

    #[test]
    fn branch_tree_creation() {
        use crate::domain::{BranchConfig, ExecutionStrategy};

        let branch = Branch::new(
            BranchId::new(),
            None,
            "session-1",
            "test-branch",
            BranchConfig::new(
                "test",
                vec![],
                ExecutionStrategy::Sequential,
                false,
                "union",
            )
            .unwrap(),
        )
        .unwrap();

        let tree = BranchTree::new(branch);
        assert_eq!(tree.total_nodes(), 1);
        assert!(tree.children.is_empty());
    }

    #[test]
    fn branch_tree_with_children() {
        use crate::domain::{BranchConfig, ExecutionStrategy};

        let root_branch = Branch::new(
            BranchId::new(),
            None,
            "session-1",
            "root-branch",
            BranchConfig::new(
                "test",
                vec![],
                ExecutionStrategy::Sequential,
                false,
                "union",
            )
            .unwrap(),
        )
        .unwrap();

        let child_branch = Branch::new(
            BranchId::new(),
            None,
            "session-2",
            "child-branch",
            BranchConfig::new(
                "test",
                vec![],
                ExecutionStrategy::Sequential,
                false,
                "union",
            )
            .unwrap(),
        )
        .unwrap();

        let mut tree = BranchTree::new(root_branch);
        tree.add_child(BranchTree::new(child_branch));

        assert_eq!(tree.total_nodes(), 2);
        assert_eq!(tree.children.len(), 1);
    }
}
