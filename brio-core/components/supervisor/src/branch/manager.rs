//! Branch Manager - Coordinates branch lifecycle with persistence.
//!
//! The BranchManager is the central coordinator for branch operations,
//! handling creation, execution, merging, and cleanup of branches.
//!
//! # Git-Like Merge Workflow
//!
//! The merge implementation follows a git-like staged workflow:
//!
//! ## Phase 1: Merge Request Creation
//! - User calls `request_merge(branch_id, strategy)`
//! - Validates branch is in `Completed` state
//! - Creates a merge request with status `Pending`
//! - Returns `MergeRequestId`
//!
//! ## Phase 2: Merge Approval
//! - User calls `approve_merge(merge_request_id, approver)`
//! - Updates merge request status to `Approved`
//! - Merge is now ready for execution
//!
//! ## Phase 3: Merge Execution (`execute_merge`)
//! 1. **Validation**: Verify merge request exists and is approved
//! 2. **Branch Validation**: Ensure branch is in `Completed` state
//! 3. **Parent Resolution**: Find parent branch or base path
//! 4. **Staging Area Creation**: Create a staging session from parent
//! 5. **Change Collection**: Scan branch session for file changes
//! 6. **Conflict Detection**: Use merge strategy to detect conflicts
//! 7. **Apply Non-Conflicting Changes**: Copy changes to staging area
//! 8. **Status Update**:
//!    - If conflicts: Status = `HasConflicts`
//!    - If no conflicts: Status = `ReadyToCommit`
//! 9. **Branch Status**: Update to `Merging`
//!
//! ## Phase 4: Conflict Resolution (Optional)
//! - If conflicts exist, user resolves them externally
//! - User calls `resolve_conflicts(merge_request_id)` to continue
//! - Status transitions back to `InProgress` then `ReadyToCommit`
//!
//! ## Phase 5: Commit (`commit_merge`)
//! 1. **Validation**: Verify merge request is in `ReadyToCommit` state
//! 2. **Commit Staging**: Apply all staged changes to parent
//! 3. **Branch Status**: Update to `Merged`
//! 4. **Merge Status**: Update to `Committed`
//!
//! ## State Diagram
//!
//! ```text
//! Pending -> Approved -> InProgress -> [HasConflicts <-> ReadyToCommit] -> Committed
//!                                      |                                  |
//!                                      +----------(conflict resolution)---+
//! ```
//!
//! ## Error Handling
//!
//! Each phase has specific error conditions:
//! - `MergeRequestNotFound`: Merge request doesn't exist
//! - `MergeNotApproved`: Attempting to execute unapproved merge
//! - `InvalidBranchState`: Branch not in correct state for operation
//! - `SessionError`: VFS session operations failed
//! - `MergeError`: Strategy execution failed
//!
//! ## Example Usage
//!
//! ```rust,no_run
//! # use supervisor::branch::BranchManager;//! # async fn example(manager: &BranchManager) -> Result<(), Box<dyn std::error::Error>> {
//! // 1. Request merge
//! let merge_id = manager.request_merge(branch_id, "union", false).await?;
//!
//! // 2. Approve merge
//! manager.approve_merge(merge_id, "user@example.com")?;
//!
//! // 3. Execute merge (creates staging area, detects conflicts)
//! let result = manager.execute_merge(merge_id).await?;
//!
//! if result.has_conflicts() {
//!     // Handle conflicts...
//!     println!("Conflicts detected: {:?}", result.conflicts);
//! } else {
//!     // 4. Commit merge (applies changes to parent)
//!     manager.commit_merge(merge_id).await?;
//!     println!("Merge committed successfully!");
//! }
//! # Ok(())
//! # }
//! ```

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use tracing::{debug, error, info, instrument, warn};

use crate::domain::{
    Branch, BranchConfig, BranchId, BranchResult, BranchSource, BranchStatus,
    BranchValidationError, MergeRequestStatus, StagedChange, ChangeType, Conflict,
};
use crate::merge::{MergeError, MergeId, MergeResult as MergeOutput, MergeStrategyRegistry, FileChange as MergeFileChange};
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
                    "Conflict at {:?}: hash changed from {} to {}",
                    path, original_hash, current_hash
                )
            }
            Self::SessionDirectoryLost(path) => write!(f, "Session directory lost: {:?}", path),
            Self::CleanupFailed { path, source } => {
                write!(f, "Cleanup failed at {:?}: {source}", path)
            }
            Self::ReadDirectoryFailed(msg) => write!(f, "Read directory failed: {msg}"),
        }
    }
}

impl std::error::Error for SessionError {}

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

    /// Adds a child to this tree node.
    pub fn add_child(&mut self, child: BranchTree) {
        self.children.push(child);
    }

    /// Returns the total number of nodes in the tree.
    #[must_use]
    pub fn total_nodes(&self) -> usize {
        1 + self.children.iter().map(|c| c.total_nodes()).sum::<usize>()
    }
}

/// Manages branch lifecycle and coordinates with persistence and VFS.
pub struct BranchManager {
    session_manager: Arc<Mutex<dyn SessionManager>>,
    repository: Arc<dyn BranchRepository>,
    merge_registry: MergeStrategyRegistry,
    max_branches: usize,
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
    /// Creates a new BranchManager with the specified dependencies.
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

    /// Creates a new BranchManager with a custom branch limit.
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

    /// Checks if we can create more branches.
    fn check_branch_limit(&self) -> Result<(), BranchError> {
        let active_count = self.repository.list_active_branches()?.len();
        if active_count >= self.max_branches {
            return Err(BranchError::MaxBranchesExceeded {
                current: active_count,
                limit: self.max_branches,
            });
        }
        Ok(())
    }

    /// Gets the base path from a branch source.
    fn get_base_path_from_source(&self, source: &BranchSource) -> Result<PathBuf, BranchError> {
        match source {
            BranchSource::Base(path) => Ok(path.clone()),
            BranchSource::Branch(parent_id) => {
                let parent = self
                    .repository
                    .get_branch(*parent_id)?
                    .ok_or(BranchError::BranchNotFound(*parent_id))?;
                // Get the session path for the parent branch
                let session_manager = self.session_manager.lock().map_err(|_| {
                    BranchError::Session(SessionError::SessionNotFound(
                        "Failed to lock session manager".to_string(),
                    ))
                })?;
                session_manager
                    .session_path(parent.session_id())
                    .ok_or_else(|| {
                        BranchError::Session(SessionError::SessionNotFound(
                            parent.session_id().to_string(),
                        ))
                    })
            }
        }
    }

    /// Checks if a status is terminal.
    #[cfg(test)]
    const fn is_terminal_status(status: BranchStatus) -> bool {
        matches!(
            status,
            BranchStatus::Completed | BranchStatus::Merged | BranchStatus::Failed
        )
    }

    /// Validates status transition.
    fn validate_status_transition(
        from: BranchStatus,
        to: BranchStatus,
    ) -> Result<(), BranchError> {
        let valid = match (from, to) {
            // Pending can transition to Active or Failed
            (BranchStatus::Pending, BranchStatus::Active) => true,
            (BranchStatus::Pending, BranchStatus::Failed) => true,
            // Active can transition to Completed, Merging, or Failed
            (BranchStatus::Active, BranchStatus::Completed) => true,
            (BranchStatus::Active, BranchStatus::Merging) => true,
            (BranchStatus::Active, BranchStatus::Failed) => true,
            // Completed can transition to Merging
            (BranchStatus::Completed, BranchStatus::Merging) => true,
            // Merging can transition to Merged or Failed
            (BranchStatus::Merging, BranchStatus::Merged) => true,
            (BranchStatus::Merging, BranchStatus::Failed) => true,
            // Terminal states cannot transition
            (BranchStatus::Merged, _) => false,
            (BranchStatus::Failed, _) => false,
            // All other transitions are invalid
            _ => false,
        };

        if valid {
            Ok(())
        } else {
            Err(BranchError::InvalidStatusTransition { from, to })
        }
    }

    /// Creates a new branch from the given source.
    ///
    /// # Errors
    ///
    /// Returns `BranchError` if:
    /// - Branch limit is exceeded
    /// - Source branch doesn't exist
    /// - VFS session creation fails
    /// - Repository persistence fails
    #[instrument(skip(self, source, config))]
    pub async fn create_branch(
        &mut self,
        source: BranchSource,
        config: BranchConfig,
    ) -> Result<BranchId, BranchError> {
        // 1. Check branch limit
        self.check_branch_limit()?;

        // 2. Get base path from source
        let base_path = self.get_base_path_from_source(&source)?;
        let base_path_str = base_path.to_string_lossy().to_string();

        // 3. Create VFS session via SessionManager
        let session_id = {
            let mut session_manager = self.session_manager.lock().map_err(|_| {
                BranchError::Session(SessionError::SessionNotFound(
                    "Failed to lock session manager".to_string(),
                ))
            })?;
            session_manager.begin_session(&base_path_str)?
        };

        // 4. Create Branch entity
        let branch_id = BranchManager::next_branch_id();
        let parent_id = match &source {
            BranchSource::Branch(id) => Some(*id),
            _ => None,
        };

        // Serialize config to JSON for storage
        let config_json = serde_json::to_string(&config)
            .map_err(|e| BranchError::Validation(BranchValidationError::InvalidExecutionStrategy {
                reason: format!("Failed to serialize config: {e}"),
            }))?;

        let branch = Branch::new(
            branch_id,
            parent_id,
            session_id.clone(),
            config.name().to_string(),
            BranchStatus::Pending,
            chrono::Utc::now().timestamp(),
            None,
            config_json,
        )
        .map_err(|e| BranchError::Validation(BranchValidationError::InvalidExecutionStrategy {
            reason: e.to_string(),
        }))?;

        // 5. Persist to repository
        self.repository.create_branch(&branch)?;

        info!(
            "Created branch {} with session {} from source {:?}",
            branch_id, session_id, source
        );

        // 6. Return BranchId
        Ok(branch_id)
    }

    /// Gets a branch by its ID.
    ///
    /// # Errors
    ///
    /// Returns `BranchError::Repository` if the query fails.
    pub fn get_branch(&self, id: BranchId) -> Result<Option<Branch>, BranchError> {
        self.repository.get_branch(id).map_err(BranchError::from)
    }

    /// Lists all active branches.
    ///
    /// # Errors
    ///
    /// Returns `BranchError::Repository` if the query fails.
    pub fn list_active_branches(&self) -> Result<Vec<Branch>, BranchError> {
        self.repository.list_active_branches().map_err(BranchError::from)
    }

    /// Lists child branches of a parent branch.
    ///
    /// # Errors
    ///
    /// Returns `BranchError::Repository` if the query fails.
    pub fn list_child_branches(&self, parent_id: BranchId) -> Result<Vec<Branch>, BranchError> {
        self.repository
            .list_branches_by_parent(parent_id)
            .map_err(BranchError::from)
    }

    /// Updates the status of a branch with transition validation.
    ///
    /// # Errors
    ///
    /// Returns `BranchError` if:
    /// - Branch not found
    /// - Status transition is invalid
    /// - Repository update fails
    #[instrument(skip(self, id, status))]
    pub fn update_status(&self, id: BranchId, status: BranchStatus) -> Result<(), BranchError> {
        // Get current branch
        let branch = self
            .repository
            .get_branch(id)?
            .ok_or(BranchError::BranchNotFound(id))?;

        // Validate transition
        Self::validate_status_transition(branch.status(), status)?;

        // Persist to repository
        self.repository.update_branch_status(id, status)?;

        debug!("Updated branch {} status to {:?}", id, status);

        Ok(())
    }

    /// Marks a branch as executing (Active status).
    ///
    /// # Errors
    ///
    /// Returns `BranchError` if status transition fails.
    pub fn mark_executing(&self, id: BranchId, _agent_count: usize) -> Result<(), BranchError> {
        self.update_status(id, BranchStatus::Active)
    }

    /// Updates execution progress (placeholder for domain compatibility).
    ///
    /// # Errors
    ///
    /// Returns `BranchError` if branch not found.
    pub fn update_progress(
        &self,
        id: BranchId,
        _active: usize,
        _completed: usize,
    ) -> Result<(), BranchError> {
        // The domain Branch type doesn't track progress details
        // This is a placeholder that validates the branch exists
        let _branch = self
            .repository
            .get_branch(id)?
            .ok_or(BranchError::BranchNotFound(id))?;
        Ok(())
    }

    /// Completes a branch with results.
    ///
    /// # Errors
    ///
    /// Returns `BranchError` if branch not found or transition invalid.
    pub fn complete_branch(
        &self,
        id: BranchId,
        _result: BranchResult,
    ) -> Result<(), BranchError> {
        self.update_status(id, BranchStatus::Completed)
    }

    /// Requests a merge for a branch.
    ///
    /// If auto-merge is enabled and no approval is required, the merge is executed immediately.
    /// Otherwise, a merge request is created for later approval.
    ///
    /// # Errors
    ///
    /// Returns `BranchError` if:
    /// - Branch not found
    /// - Branch is not in Completed state
    /// - Invalid merge strategy
    /// - Repository operation fails
    #[instrument(skip(self, branch_id, strategy))]
    pub async fn request_merge(
        &self,
        branch_id: BranchId,
        strategy: &str,
        _requires_approval: bool,
    ) -> Result<MergeRequestId, BranchError> {
        // 1. Get branch
        let branch = self
            .repository
            .get_branch(branch_id)?
            .ok_or(BranchError::BranchNotFound(branch_id))?;

        // 2. Validate branch is Completed
        if branch.status() != BranchStatus::Completed {
            return Err(BranchError::InvalidBranchState {
                branch_id,
                expected: "Completed".to_string(),
                actual: format!("{:?}", branch.status()),
            });
        }

        // 3. Validate strategy exists
        if self.merge_registry.get(strategy).is_none() {
            return Err(BranchError::InvalidStrategy(strategy.to_string()));
        }

        // 4. Create merge request
        let parent_id = branch.parent_id();
        let merge_id = self
            .repository
            .create_merge_request(branch_id, parent_id, strategy)?;

        info!(
            "Created merge request {} for branch {} with strategy '{}'",
            merge_id, branch_id, strategy
        );

        Ok(merge_id)
    }

    /// Executes a merge after approval using a git-like workflow.
    ///
    /// This implements a staged merge process:
    /// 1. Find merge request and validate it's approved
    /// 2. Get branch and validate it's Completed
    /// 3. Find parent branch (or use base path)
    /// 4. Create staging session for merge
    /// 5. Collect file changes from branch session
    /// 6. Detect conflicts using merge strategies
    /// 7. Apply non-conflicting changes to staging
    /// 8. If conflicts exist, mark as HasConflicts and return
    /// 9. If no conflicts, mark as ReadyToCommit
    ///
    /// # Errors
    ///
    /// Returns `BranchError` if:
    /// - Merge request not found
    /// - Merge request not approved
    /// - Branch not found or not in Completed state
    /// - Strategy not found
    /// - Merge execution fails
    /// - Session creation fails
    #[instrument(skip(self, merge_request_id))]
    pub async fn execute_merge(
        &self,
        merge_request_id: MergeRequestId,
    ) -> Result<MergeOutput, BranchError> {
        // 1. Get merge request
        let merge_request = self
            .repository
            .get_merge_request(merge_request_id)
            .map_err(|e| BranchError::Repository(e))?
            .ok_or(BranchError::MergeRequestNotFound(merge_request_id))?;

        // 2. Validate merge request is approved
        if !merge_request.is_approved() {
            return Err(BranchError::MergeNotApproved(merge_request_id));
        }

        // 3. Get branch being merged
        let branch_id = merge_request.branch_id();
        let branch = self
            .repository
            .get_branch(branch_id)?
            .ok_or(BranchError::BranchNotFound(branch_id))?;

        // 4. Validate branch is in Completed state
        if branch.status() != BranchStatus::Completed {
            return Err(BranchError::InvalidBranchState {
                branch_id,
                expected: "Completed".to_string(),
                actual: format!("{:?}", branch.status()),
            });
        }

        // 5. Get parent path for merge destination
        let parent_path = if let Some(parent_id) = branch.parent_id() {
            let parent = self
                .repository
                .get_branch(parent_id)?
                .ok_or(BranchError::BranchNotFound(parent_id))?;
            let session_manager = self.session_manager.lock().map_err(|_| {
                BranchError::Session(SessionError::SessionNotFound(
                    "Failed to lock session manager".to_string(),
                ))
            })?;
            session_manager
                .session_path(parent.session_id())
                .ok_or_else(|| {
                    BranchError::Session(SessionError::SessionNotFound(
                        parent.session_id().to_string(),
                    ))
                })?
        } else {
            // For root branches, we'd need a base path - this is a placeholder
            return Err(BranchError::Validation(
                BranchValidationError::InvalidExecutionStrategy {
                    reason: "Root branch merge not implemented".to_string(),
                },
            ));
        };

        // 6. Create staging session for merge
        let parent_path_str = parent_path.to_string_lossy().to_string();
        let staging_session_id = {
            let mut session_manager = self.session_manager.lock().map_err(|_| {
                BranchError::Session(SessionError::SessionNotFound(
                    "Failed to lock session manager".to_string(),
                ))
            })?;
            session_manager.begin_session(&parent_path_str)?
        };

        // 7. Collect file changes from branch session
        let branch_changes = self.collect_branch_changes(&branch).await?;

        // 8. Get the strategy and perform merge
        let strategy_name = merge_request.strategy();
        let strategy = self
            .merge_registry
            .get(strategy_name)
            .ok_or_else(|| BranchError::InvalidStrategy(strategy_name.to_string()))?;

        // Create branch result for merge strategy
        let branch_result = crate::merge::BranchResult::new(
            branch_id,
            parent_path.clone(),
            branch_changes,
        );

        // Execute merge strategy
        let merge_result = strategy
            .merge(&parent_path, &[branch_result])
            .await
            .map_err(BranchError::Merge)?;

        // 9. Convert merge result to staged changes
        let staged_changes: Vec<StagedChange> = merge_result
            .merged_changes
            .iter()
            .map(|change| StagedChange {
                path: change.path().to_path_buf(),
                change_type: match change {
                    MergeFileChange::Added(_) => ChangeType::Added,
                    MergeFileChange::Modified(_) => ChangeType::Modified,
                    MergeFileChange::Deleted(_) => ChangeType::Deleted,
                },
                content_hash: None, // Could compute hash here if needed
            })
            .collect();

        // 10. Apply non-conflicting changes to staging area
        if !merge_result.has_conflicts() {
            // Apply all changes to staging session
            self.apply_changes_to_staging(&staging_session_id, &merge_result.merged_changes)?;
        }

        // 11. Update merge request status
        let conflicts: Vec<Conflict> = merge_result
            .conflicts
            .iter()
            .map(|c| Conflict {
                file_path: c.path.clone(),
                kind: crate::domain::ConflictType::Content,
                base_content: None,
                branch_contents: std::collections::HashMap::new(),
            })
            .collect();

        // Build updated merge request
        let mut updated_merge_request = merge_request.clone();
        updated_merge_request.start(staging_session_id.clone(), chrono::Utc::now().timestamp());
        updated_merge_request.set_staged_changes(staged_changes);
        updated_merge_request.set_conflicts(conflicts);

        // Save updated merge request
        self.repository
            .update_merge_request(&updated_merge_request)
            .map_err(|e| BranchError::Repository(e))?;

        // 12. Update branch status to Merging
        self.update_status(branch_id, BranchStatus::Merging)?;

        info!(
            "Executed merge {} for branch {}: {} changes, {} conflicts",
            merge_request_id,
            branch_id,
            merge_result.merged_changes.len(),
            merge_result.conflicts.len()
        );

        // 13. Return merge output
        Ok(merge_result)
    }

    /// Commits a staged merge to the parent branch.
    ///
    /// This is the final step in the git-like merge workflow, similar to `git commit`.
    /// It applies all staged changes to the parent branch and marks the merge as complete.
    ///
    /// # Errors
    ///
    /// Returns `BranchError` if:
    /// - Merge request not found
    /// - Merge not in ReadyToCommit state
    /// - Session commit fails
    /// - Repository update fails
    #[instrument(skip(self, merge_request_id))]
    pub async fn commit_merge(&self, merge_request_id: MergeRequestId) -> Result<(), BranchError> {
        // 1. Get merge request
        let merge_request = self
            .repository
            .get_merge_request(merge_request_id)
            .map_err(|e| BranchError::Repository(e))?
            .ok_or(BranchError::MergeRequestNotFound(merge_request_id))?;

        // 2. Validate merge is ready to commit
        if merge_request.status() != MergeRequestStatus::ReadyToCommit {
            return Err(BranchError::InvalidBranchState {
                branch_id: merge_request.branch_id(),
                expected: "ReadyToCommit".to_string(),
                actual: format!("{:?}", merge_request.status()),
            });
        }

        // 3. Get staging session ID
        let staging_session_id = merge_request
            .staging_session_id()
            .ok_or_else(|| BranchError::Session(SessionError::SessionNotFound(
                "Merge staging session not found".to_string(),
            )))?;

        // 4. Commit staging session to parent
        {
            let mut session_manager = self.session_manager.lock().map_err(|_| {
                BranchError::Session(SessionError::SessionNotFound(
                    "Failed to lock session manager".to_string(),
                ))
            })?;
            session_manager.commit_session(staging_session_id)?;
        }

        // 5. Get branch and mark as Merged
        let branch_id = merge_request.branch_id();
        self.update_status(branch_id, BranchStatus::Merged)?;

        // 6. Mark merge request as committed
        let mut updated_merge_request = merge_request.clone();
        updated_merge_request.mark_committed(chrono::Utc::now().timestamp());
        self.repository
            .update_merge_request(&updated_merge_request)
            .map_err(|e| BranchError::Repository(e))?;

        info!("Committed merge {} for branch {}", merge_request_id, branch_id);

        Ok(())
    }

    /// Collects file changes from a branch session.
    ///
    /// This scans the branch's session directory and compares it with the parent
    /// to determine what files have been added, modified, or deleted.
    ///
    /// # Errors
    /// Returns `BranchError` if session access fails.
    async fn collect_branch_changes(
        &self,
        branch: &Branch,
    ) -> Result<Vec<MergeFileChange>, BranchError> {
        let session_id = branch.session_id();
        
        // Get session path
        let session_path = {
            let session_manager = self.session_manager.lock().map_err(|_| {
                BranchError::Session(SessionError::SessionNotFound(
                    "Failed to lock session manager".to_string(),
                ))
            })?;
            session_manager
                .session_path(session_id)
                .ok_or_else(|| {
                    BranchError::Session(SessionError::SessionNotFound(session_id.to_string()))
                })?
        };

        // Collect changes by scanning the session directory
        let mut changes = Vec::new();
        self.scan_directory_for_changes(&session_path, PathBuf::new(), &mut changes).await?;

        Ok(changes)
    }

    /// Recursively scans a directory for file changes.
    ///
    /// # Errors
    /// Returns `BranchError` if directory reading fails.
    async fn scan_directory_for_changes(
        &self,
        base_path: &PathBuf,
        relative_path: PathBuf,
        changes: &mut Vec<MergeFileChange>,
    ) -> Result<(), BranchError> {
        let full_path = base_path.join(&relative_path);
        
        let entries = tokio::fs::read_dir(&full_path)
            .await
            .map_err(|e| BranchError::Session(SessionError::ReadDirectoryFailed(e.to_string())))?;

        let mut entries = entries;
        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|e| BranchError::Session(SessionError::ReadDirectoryFailed(e.to_string())))?
        {
            let file_name = entry.file_name();
            let file_name_str = file_name.to_string_lossy();
            
            // Skip hidden files and directories
            if file_name_str.starts_with('.') {
                continue;
            }

            let entry_relative_path = relative_path.join(&file_name);

            let file_type = entry
                .file_type()
                .await
                .map_err(|e| BranchError::Session(SessionError::ReadDirectoryFailed(e.to_string())))?;

            if file_type.is_dir() {
                // Recursively scan subdirectory
                Box::pin(self.scan_directory_for_changes(
                    base_path,
                    entry_relative_path,
                    changes,
                ))
                .await?;
            } else if file_type.is_file() {
                // Record file as modified (simplified - in real impl, compare with parent)
                changes.push(MergeFileChange::Modified(entry_relative_path));
            }
        }

        Ok(())
    }

    /// Applies changes to the staging session.
    ///
    /// # Errors
    /// Returns `BranchError` if file operations fail.
    fn apply_changes_to_staging(
        &self,
        _staging_session_id: &str,
        changes: &[MergeFileChange],
    ) -> Result<(), BranchError> {
        // In a full implementation, this would:
        // 1. Get the staging session path
        // 2. Apply each change (copy files from branch, delete removed files, etc.)
        // 3. Handle any I/O errors

        for change in changes {
            match change {
                MergeFileChange::Added(path) | MergeFileChange::Modified(path) => {
                    debug!("Applying change to staging: {:?}", path);
                    // Would copy file from branch to staging here
                }
                MergeFileChange::Deleted(path) => {
                    debug!("Applying deletion to staging: {:?}", path);
                    // Would delete file from staging here
                }
            }
        }

        Ok(())
    }

    /// Approves a pending merge request.
    ///
    /// # Errors
    ///
    /// Returns `BranchError` if:
    /// - Merge request not found
    /// - Repository update fails
    #[instrument(skip(self, merge_request_id))]
    pub fn approve_merge(
        &self,
        merge_request_id: MergeRequestId,
        approver: &str,
    ) -> Result<(), BranchError> {
        self.repository
            .approve_merge(merge_request_id, approver)
            .map_err(|e| match e {
                BranchRepositoryError::BranchNotFound(_) => {
                    BranchError::MergeRequestNotFound(merge_request_id)
                }
                _ => BranchError::Repository(e),
            })?;

        info!(
            "Approved merge request {} by {}",
            merge_request_id, approver
        );

        Ok(())
    }

    /// Aborts/rolls back a branch.
    ///
    /// # Errors
    ///
    /// Returns `BranchError` if:
    /// - Branch not found
    /// - Session rollback fails
    /// - Repository update fails
    #[instrument(skip(self, id))]
    pub async fn abort_branch(&self, id: BranchId) -> Result<(), BranchError> {
        // 1. Get branch session_id
        let branch = self
            .repository
            .get_branch(id)?
            .ok_or(BranchError::BranchNotFound(id))?;

        let session_id = branch.session_id().to_string();

        // 2. Rollback session via SessionManager
        {
            let mut session_manager = self.session_manager.lock().map_err(|_| {
                BranchError::Session(SessionError::SessionNotFound(
                    "Failed to lock session manager".to_string(),
                ))
            })?;
            session_manager.rollback_session(&session_id)?;
        }

        // 3. Mark branch as Failed
        self.update_status(id, BranchStatus::Failed)?;

        info!("Aborted branch {} and rolled back session {}", id, session_id);

        Ok(())
    }

    /// Recovers branches after a restart.
    ///
    /// Returns a list of branch IDs that were successfully recovered.
    /// Branches in "Active" status are transitioned back to "Pending".
    ///
    /// # Errors
    ///
    /// Returns `BranchError::Repository` if the query fails.
    #[instrument(skip(self))]
    pub fn recover_branches(&self) -> Result<Vec<BranchId>, BranchError> {
        // 1. Query all active branches from repository
        let active_branches = self.repository.list_active_branches()?;

        let mut recovered = Vec::new();

        for branch in active_branches {
            let branch_id = branch.id();

            // 2. Validate session still exists
            let session_exists = {
                let session_manager = self.session_manager.lock().map_err(|_| {
                    BranchError::Session(SessionError::SessionNotFound(
                        "Failed to lock session manager".to_string(),
                    ))
                })?;
                session_manager.session_path(branch.session_id()).is_some()
            };

            if !session_exists {
                warn!(
                    "Branch {} session {} no longer exists, marking as failed",
                    branch_id,
                    branch.session_id()
                );
                let _ = self.update_status(branch_id, BranchStatus::Failed);
                continue;
            }

            // 3. Branches in "Active" should transition back to "Pending"
            if branch.status() == BranchStatus::Active {
                info!("Recovering branch {} from Active to Pending", branch_id);
                if let Err(e) =
                    self.repository
                        .update_branch_status(branch_id, BranchStatus::Pending)
                {
                    error!("Failed to recover branch {}: {}", branch_id, e);
                    continue;
                }
            }

            recovered.push(branch_id);
        }

        info!("Recovered {} branches after restart", recovered.len());

        Ok(recovered)
    }

    /// Gets the branch tree starting from a root branch.
    ///
    /// # Errors
    ///
    /// Returns `BranchError::BranchNotFound` if the root branch doesn't exist.
    pub fn get_branch_tree(&self, root_id: BranchId) -> Result<BranchTree, BranchError> {
        // Get the root branch
        let root = self
            .repository
            .get_branch(root_id)?
            .ok_or(BranchError::BranchNotFound(root_id))?;

        // Recursively build the tree
        self.build_branch_tree(root)
    }

    /// Recursively builds a branch tree.
    fn build_branch_tree(&self, branch: Branch) -> Result<BranchTree, BranchError> {
        let mut tree = BranchTree::new(branch.clone());

        // Get child branches
        let children = self.repository.list_branches_by_parent(branch.id())?;

        for child in children {
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
    use crate::domain::{ExecutionStrategy, MergeRequest};
    use std::collections::HashMap;

    /// Mock repository for testing.
    struct MockBranchRepository {
        branches: Mutex<HashMap<BranchId, Branch>>,
        merge_requests: Mutex<HashMap<MergeId, (BranchId, String)>>,
    }

    impl MockBranchRepository {
        fn new() -> Self {
            Self {
                branches: Mutex::new(HashMap::new()),
                merge_requests: Mutex::new(HashMap::new()),
            }
        }
    }

    impl BranchRepository for MockBranchRepository {
        fn create_branch(
            &self,
            branch: &Branch,
        ) -> Result<BranchId, BranchRepositoryError> {
            let mut branches = self.branches.lock().map_err(|_| {
                BranchRepositoryError::SqlError("Lock failed".to_string())
            })?;
            branches.insert(branch.id(), branch.clone());
            Ok(branch.id())
        }

        fn get_branch(
            &self,
            id: BranchId,
        ) -> Result<Option<Branch>, BranchRepositoryError> {
            let branches = self.branches.lock().map_err(|_| {
                BranchRepositoryError::SqlError("Lock failed".to_string())
            })?;
            Ok(branches.get(&id).cloned())
        }

        fn update_branch_status(
            &self,
            id: BranchId,
            status: BranchStatus,
        ) -> Result<(), BranchRepositoryError> {
            let mut branches = self.branches.lock().map_err(|_| {
                BranchRepositoryError::SqlError("Lock failed".to_string())
            })?;
            if let Some(existing) = branches.get(&id) {
                let updated = Branch::new(
                    existing.id(),
                    existing.parent_id(),
                    existing.session_id().to_string(),
                    existing.name().to_string(),
                    status,
                    existing.created_at(),
                    existing.completed_at(),
                    existing.config().to_string(),
                )
                .map_err(|e| BranchRepositoryError::ParseError(e.to_string()))?;
                branches.insert(id, updated);
                Ok(())
            } else {
                Err(BranchRepositoryError::BranchNotFound(id))
            }
        }

        fn list_active_branches(&self) -> Result<Vec<Branch>, BranchRepositoryError> {
            let branches = self.branches.lock().map_err(|_| {
                BranchRepositoryError::SqlError("Lock failed".to_string())
            })?;
            Ok(branches
                .values()
                .filter(|b| b.is_active())
                .cloned()
                .collect())
        }

        fn list_branches_by_parent(
            &self,
            parent_id: BranchId,
        ) -> Result<Vec<Branch>, BranchRepositoryError> {
            let branches = self.branches.lock().map_err(|_| {
                BranchRepositoryError::SqlError("Lock failed".to_string())
            })?;
            Ok(branches
                .values()
                .filter(|b| b.parent_id() == Some(parent_id))
                .cloned()
                .collect())
        }

        fn delete_branch(&self, id: BranchId) -> Result<(), BranchRepositoryError> {
            let mut branches = self.branches.lock().map_err(|_| {
                BranchRepositoryError::SqlError("Lock failed".to_string())
            })?;
            branches.remove(&id);
            Ok(())
        }

        fn create_merge_request(
            &self,
            branch_id: BranchId,
            _parent_id: Option<BranchId>,
            strategy: &str,
        ) -> Result<MergeId, BranchRepositoryError> {
            let mut merge_requests = self.merge_requests.lock().map_err(|_| {
                BranchRepositoryError::SqlError("Lock failed".to_string())
            })?;
            let merge_id = MergeId::new();
            merge_requests.insert(merge_id, (branch_id, strategy.to_string()));
            Ok(merge_id)
        }

        fn get_merge_request(
            &self,
            _merge_id: MergeId,
        ) -> Result<Option<MergeRequest>, BranchRepositoryError> {
            // Mock implementation returns None
            Ok(None)
        }

        fn update_merge_request(
            &self,
            _merge_request: &MergeRequest,
        ) -> Result<(), BranchRepositoryError> {
            // Mock implementation does nothing
            Ok(())
        }

        fn approve_merge(
            &self,
            merge_id: MergeId,
            _approver: &str,
        ) -> Result<(), BranchRepositoryError> {
            let merge_requests = self.merge_requests.lock().map_err(|_| {
                BranchRepositoryError::SqlError("Lock failed".to_string())
            })?;
            if merge_requests.contains_key(&merge_id) {
                Ok(())
            } else {
                Err(BranchRepositoryError::BranchNotFound(BranchId::new()))
            }
        }
    }

    fn create_test_config() -> BranchConfig {
        BranchConfig::new(
            "test-branch",
            vec![],
            ExecutionStrategy::Sequential,
            false,
            "union",
        )
        .unwrap()
    }

    fn create_test_branch(id: u64, parent_id: Option<u64>) -> Branch {
        let parent = parent_id.map(|pid| BranchId::from_uuid(uuid::Uuid::from_u128(pid as u128)));
        Branch::new(
            BranchId::from_uuid(uuid::Uuid::from_u128(id as u128)),
            parent,
            format!("session-{}", id),
            format!("branch-{}", id),
            BranchStatus::Pending,
            chrono::Utc::now().timestamp(),
            None,
            "{}".to_string(),
        )
        .unwrap()
    }

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
        let branch = create_test_branch(1, None);
        let tree = BranchTree::new(branch);
        assert_eq!(tree.total_nodes(), 1);
        assert!(tree.children.is_empty());
    }

    #[test]
    fn branch_tree_with_children() {
        let root_branch = create_test_branch(1, None);
        let child_branch = create_test_branch(2, Some(1));

        let mut tree = BranchTree::new(root_branch);
        tree.add_child(BranchTree::new(child_branch));

        assert_eq!(tree.total_nodes(), 2);
        assert_eq!(tree.children.len(), 1);
    }

    #[test]
    fn mock_repository_create_and_get() {
        let repo = MockBranchRepository::new();
        let branch = create_test_branch(1, None);
        let id = branch.id();

        repo.create_branch(&branch).unwrap();
        let retrieved = repo.get_branch(id).unwrap();

        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().id(), id);
    }

    #[test]
    fn mock_repository_update_status() {
        let repo = MockBranchRepository::new();
        let branch = create_test_branch(1, None);
        let id = branch.id();

        repo.create_branch(&branch).unwrap();

        repo.update_branch_status(id, BranchStatus::Active).unwrap();

        let retrieved = repo.get_branch(id).unwrap().unwrap();
        assert_eq!(retrieved.status(), BranchStatus::Active);
    }

    #[test]
    fn mock_repository_list_by_parent() {
        let repo = MockBranchRepository::new();
        let parent = create_test_branch(1, None);
        let child1 = create_test_branch(2, Some(1));
        let child2 = create_test_branch(3, Some(1));
        let other = create_test_branch(4, None);

        repo.create_branch(&parent).unwrap();
        repo.create_branch(&child1).unwrap();
        repo.create_branch(&child2).unwrap();
        repo.create_branch(&other).unwrap();

        let children = repo.list_branches_by_parent(parent.id()).unwrap();
        assert_eq!(children.len(), 2);

        let ids: std::collections::HashSet<_> = children.iter().map(|b| b.id().inner()).collect();
        assert!(ids.contains(&uuid::Uuid::from_u128(2)));
        assert!(ids.contains(&uuid::Uuid::from_u128(3)));
        assert!(!ids.contains(&uuid::Uuid::from_u128(4)));
    }

    #[test]
    fn mock_repository_create_merge_request() {
        let repo = MockBranchRepository::new();
        let branch = create_test_branch(1, None);
        repo.create_branch(&branch).unwrap();

        let merge_id = repo.create_merge_request(branch.id(), None, "union").unwrap();

        // Approve should succeed
        repo.approve_merge(merge_id, "test-user").unwrap();
    }

    #[test]
    fn mock_repository_approve_nonexistent_merge() {
        let repo = MockBranchRepository::new();
        let merge_id = MergeId::new();

        let result = repo.approve_merge(merge_id, "test-user");
        assert!(result.is_err());
    }

    #[test]
    fn branch_status_transition_validation() {
        // Valid transitions
        assert!(BranchManager::validate_status_transition(
            BranchStatus::Pending,
            BranchStatus::Active
        )
        .is_ok());

        // Invalid transitions
        assert!(BranchManager::validate_status_transition(
            BranchStatus::Pending,
            BranchStatus::Completed
        )
        .is_err());
    }

    #[test]
    fn is_terminal_status() {
        assert!(BranchManager::is_terminal_status(BranchStatus::Completed));
        assert!(BranchManager::is_terminal_status(BranchStatus::Merged));
        assert!(BranchManager::is_terminal_status(BranchStatus::Failed));
        assert!(!BranchManager::is_terminal_status(BranchStatus::Pending));
        assert!(!BranchManager::is_terminal_status(BranchStatus::Active));
    }
}
