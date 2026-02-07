//! Domain Layer - Value Objects and Entities
//!
//! This module defines the core domain types for the Supervisor.
//! All types are explicit, self-documenting, and follow the principle of
//! making invalid states unrepresentable.

use core::fmt;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

/// Maximum number of concurrent branches allowed.
pub const MAX_CONCURRENT_BRANCHES: usize = 8;

/// Minimum length for branch names.
pub const MIN_BRANCH_NAME_LEN: usize = 1;

/// Maximum length for branch names.
pub const MAX_BRANCH_NAME_LEN: usize = 256;

/// Error type for domain validation failures.
#[derive(Debug, Clone)]
pub enum ValidationError {
    /// AgentId cannot be empty.
    EmptyAgentId,
    /// Task content cannot be empty.
    EmptyTaskContent,
    /// Branch name cannot be empty.
    EmptyBranchName,
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyAgentId => write!(f, "AgentId cannot be empty"),
            Self::EmptyTaskContent => write!(f, "Task content cannot be empty"),
            Self::EmptyBranchName => write!(f, "Branch name cannot be empty"),
        }
    }
}

impl std::error::Error for ValidationError {}

/// Error type for branch validation failures.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BranchValidationError {
    /// Branch name is empty or too short.
    InvalidNameLength {
        /// The actual length of the name.
        len: usize,
        /// The minimum allowed length.
        min: usize,
        /// The maximum allowed length.
        max: usize,
    },
    /// Session ID is empty.
    EmptySessionId,
    /// Maximum concurrent branches exceeded.
    MaxConcurrentBranchesExceeded {
        /// The number of branches requested.
        requested: usize,
        /// The maximum allowed branches.
        max: usize,
    },
    /// Invalid execution strategy configuration.
    InvalidExecutionStrategy {
        /// The reason the strategy is invalid.
        reason: String,
    },
    /// Cannot transition from current status.
    InvalidStatusTransition {
        /// The current status.
        from: BranchStatusKind,
        /// The target status.
        to: BranchStatusKind,
    },
    /// Agent assignment is invalid.
    InvalidAgentAssignment {
        /// The reason the assignment is invalid.
        reason: String,
    },
}

impl fmt::Display for BranchValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidNameLength { len, min, max } => {
                write!(
                    f,
                    "Branch name length {} is outside valid range [{}-{}]",
                    len, min, max
                )
            }
            Self::EmptySessionId => write!(f, "Session ID cannot be empty"),
            Self::MaxConcurrentBranchesExceeded { requested, max } => {
                write!(
                    f,
                    "Requested {} concurrent branches, but maximum is {}",
                    requested, max
                )
            }
            Self::InvalidExecutionStrategy { reason } => {
                write!(f, "Invalid execution strategy: {}", reason)
            }
            Self::InvalidStatusTransition { from, to } => {
                write!(f, "Cannot transition from {:?} to {:?}", from, to)
            }
            Self::InvalidAgentAssignment { reason } => {
                write!(f, "Invalid agent assignment: {}", reason)
            }
        }
    }
}

impl std::error::Error for BranchValidationError {}

impl From<ValidationError> for BranchValidationError {
    fn from(err: ValidationError) -> Self {
        match err {
            ValidationError::EmptyAgentId => Self::InvalidAgentAssignment {
                reason: "Agent ID cannot be empty".to_string(),
            },
            ValidationError::EmptyTaskContent => Self::InvalidAgentAssignment {
                reason: "Task override cannot be empty".to_string(),
            },
            ValidationError::EmptyBranchName => Self::InvalidNameLength {
                len: 0,
                min: MIN_BRANCH_NAME_LEN,
                max: MAX_BRANCH_NAME_LEN,
            },
        }
    }
}

/// Unique identifier for a branch in the system.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BranchId(
    /// The underlying UUID.
    pub uuid::Uuid,
);

impl BranchId {
    /// Creates a new `BranchId` with a random UUID.
    #[must_use]
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4())
    }

    /// Creates a `BranchId` from an existing UUID.
    #[must_use]
    pub const fn from_uuid(uuid: uuid::Uuid) -> Self {
        Self(uuid)
    }

    /// Returns the inner UUID.
    #[must_use]
    pub const fn inner(self) -> uuid::Uuid {
        self.0
    }
}

impl Default for BranchId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for BranchId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Immutable branch entity representing an isolated execution context.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Branch {
    id: BranchId,
    parent_id: Option<BranchId>,
    session_id: String,
    name: String,
    status: BranchStatus,
    created_at: i64,
    completed_at: Option<i64>,
    config: String,
}

impl Branch {
    /// Constructs a new Branch (factory method).
    ///
    /// # Errors
    /// Returns `ValidationError::EmptyBranchName` if name is empty.
    pub fn new(
        id: BranchId,
        parent_id: Option<BranchId>,
        session_id: String,
        name: String,
        status: BranchStatus,
        created_at: i64,
        completed_at: Option<i64>,
        config: String,
    ) -> Result<Self, ValidationError> {
        if name.is_empty() {
            return Err(ValidationError::EmptyBranchName);
        }
        Ok(Self {
            id,
            parent_id,
            session_id,
            name,
            status,
            created_at,
            completed_at,
            config,
        })
    }

    /// Returns the branch ID.
    #[must_use]
    pub const fn id(&self) -> BranchId {
        self.id
    }

    /// Returns the parent branch ID, if any.
    #[must_use]
    pub const fn parent_id(&self) -> Option<BranchId> {
        self.parent_id
    }

    /// Returns the session ID.
    #[must_use]
    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    /// Returns the branch name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the current branch status.
    #[must_use]
    pub const fn status(&self) -> BranchStatus {
        self.status
    }

    /// Returns the creation timestamp.
    #[must_use]
    pub const fn created_at(&self) -> i64 {
        self.created_at
    }

    /// Returns the completion timestamp, if any.
    #[must_use]
    pub const fn completed_at(&self) -> Option<i64> {
        self.completed_at
    }

    /// Returns the branch configuration JSON.
    #[must_use]
    pub fn config(&self) -> &str {
        &self.config
    }

    /// Checks if this branch is active (managed by supervisor).
    #[must_use]
    pub fn is_active(&self) -> bool {
        matches!(
            self.status,
            BranchStatus::Pending | BranchStatus::Active | BranchStatus::Merging
        )
    }
}

/// Branch lifecycle status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BranchStatus {
    /// Branch is created but not yet active.
    Pending,
    /// Branch is actively being executed.
    Active,
    /// Branch completed successfully.
    Completed,
    /// Branch failed during execution.
    Failed,
    /// Branch is currently being merged.
    Merging,
    /// Branch has been merged.
    Merged,
}

impl BranchStatus {
    /// Checks if this is a terminal status.
    #[must_use]
    pub const fn is_terminal(&self) -> bool {
        matches!(self, Self::Completed | Self::Merged | Self::Failed)
    }

    /// Checks if the branch is in an active state.
    #[must_use]
    pub const fn is_active(&self) -> bool {
        matches!(self, Self::Active | Self::Merging)
    }
}

/// Simplified branch status kind for error messages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BranchStatusKind {
    /// Branch is pending creation.
    Pending,
    /// Branch is currently active.
    Active,
    /// Branch has completed.
    Completed,
    /// Branch is currently being merged.
    Merging,
    /// Branch has been merged.
    Merged,
    /// Branch has failed.
    Failed,
}

impl From<&BranchStatus> for BranchStatusKind {
    fn from(status: &BranchStatus) -> Self {
        match status {
            BranchStatus::Pending => Self::Pending,
            BranchStatus::Active => Self::Active,
            BranchStatus::Completed => Self::Completed,
            BranchStatus::Failed => Self::Failed,
            BranchStatus::Merging => Self::Merging,
            BranchStatus::Merged => Self::Merged,
        }
    }
}

/// Execution strategy for running branch tasks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExecutionStrategy {
    /// Execute tasks sequentially, one at a time.
    Sequential,
    /// Execute tasks in parallel with a maximum concurrency limit.
    Parallel {
        /// The maximum number of concurrent tasks allowed.
        max_concurrent: usize,
    },
}

impl ExecutionStrategy {
    /// Validates the execution strategy configuration.
    ///
    /// # Errors
    /// Returns `BranchValidationError::InvalidExecutionStrategy` if max_concurrent exceeds limit.
    pub fn validate(&self) -> Result<(), BranchValidationError> {
        match self {
            Self::Sequential => Ok(()),
            Self::Parallel { max_concurrent } => {
                if *max_concurrent == 0 {
                    Err(BranchValidationError::InvalidExecutionStrategy {
                        reason: "max_concurrent must be at least 1".to_string(),
                    })
                } else if *max_concurrent > MAX_CONCURRENT_BRANCHES {
                    Err(BranchValidationError::InvalidExecutionStrategy {
                        reason: format!("max_concurrent cannot exceed {}", MAX_CONCURRENT_BRANCHES),
                    })
                } else {
                    Ok(())
                }
            }
        }
    }

    /// Returns the effective concurrency limit.
    #[must_use]
    pub const fn concurrency_limit(&self) -> usize {
        match self {
            Self::Sequential => 1,
            Self::Parallel { max_concurrent } => *max_concurrent,
        }
    }
}

impl Default for ExecutionStrategy {
    fn default() -> Self {
        Self::Sequential
    }
}

/// Assignment of an agent to a branch with optional overrides.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentAssignment {
    agent_id: AgentId,
    task_override: Option<String>,
    priority: Priority,
}

impl AgentAssignment {
    /// Creates a new agent assignment.
    ///
    /// # Errors
    /// Returns `ValidationError` if agent_id is empty.
    pub fn new(
        agent_id: impl Into<String>,
        task_override: Option<String>,
        priority: Priority,
    ) -> Result<Self, ValidationError> {
        Ok(Self {
            agent_id: AgentId::new(agent_id)?,
            task_override,
            priority,
        })
    }

    /// Returns the agent ID.
    #[must_use]
    pub fn agent_id(&self) -> &AgentId {
        &self.agent_id
    }

    /// Returns the task override, if any.
    #[must_use]
    pub fn task_override(&self) -> Option<&str> {
        self.task_override.as_deref()
    }

    /// Returns the priority for this agent's tasks.
    #[must_use]
    pub const fn priority(&self) -> Priority {
        self.priority
    }
}

/// Configuration for branch execution.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BranchConfig {
    name: String,
    agents: Vec<AgentAssignment>,
    execution_strategy: ExecutionStrategy,
    auto_merge: bool,
    merge_strategy: String,
}

impl BranchConfig {
    /// Creates a new branch configuration with validation.
    ///
    /// # Errors
    /// Returns `BranchValidationError` if validation fails.
    pub fn new(
        name: impl Into<String>,
        agents: Vec<AgentAssignment>,
        execution_strategy: ExecutionStrategy,
        auto_merge: bool,
        merge_strategy: impl Into<String>,
    ) -> Result<Self, BranchValidationError> {
        let name = name.into();
        let name_len = name.len();

        if name_len < MIN_BRANCH_NAME_LEN || name_len > MAX_BRANCH_NAME_LEN {
            return Err(BranchValidationError::InvalidNameLength {
                len: name_len,
                min: MIN_BRANCH_NAME_LEN,
                max: MAX_BRANCH_NAME_LEN,
            });
        }

        execution_strategy.validate()?;

        Ok(Self {
            name,
            agents,
            execution_strategy,
            auto_merge,
            merge_strategy: merge_strategy.into(),
        })
    }

    /// Returns the name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the agents.
    #[must_use]
    pub fn agents(&self) -> &[AgentAssignment] {
        &self.agents
    }

    /// Returns the execution strategy.
    #[must_use]
    pub const fn execution_strategy(&self) -> ExecutionStrategy {
        self.execution_strategy
    }

    /// Returns whether auto-merge is enabled.
    #[must_use]
    pub const fn auto_merge(&self) -> bool {
        self.auto_merge
    }

    /// Returns the merge strategy.
    #[must_use]
    pub fn merge_strategy(&self) -> &str {
        &self.merge_strategy
    }
}

impl Default for BranchConfig {
    fn default() -> Self {
        Self {
            name: "default-branch".to_string(),
            agents: Vec::new(),
            execution_strategy: ExecutionStrategy::default(),
            auto_merge: false,
            merge_strategy: "three-way".to_string(),
        }
    }
}

/// Type of file change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChangeType {
    /// File was added.
    Added,
    /// File was modified.
    Modified,
    /// File was deleted.
    Deleted,
    /// File was renamed.
    Renamed,
}

/// A single file change.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FileChange {
    path: PathBuf,
    change_type: ChangeType,
    diff: Option<String>,
}

impl FileChange {
    /// Creates a new file change.
    #[must_use]
    pub fn new(path: PathBuf, change_type: ChangeType, diff: Option<String>) -> Self {
        Self {
            path,
            change_type,
            diff,
        }
    }

    /// Returns the path.
    #[must_use]
    pub fn path(&self) -> &PathBuf {
        &self.path
    }

    /// Returns the change type.
    #[must_use]
    pub const fn change_type(&self) -> ChangeType {
        self.change_type
    }

    /// Returns the diff.
    #[must_use]
    pub fn diff(&self) -> Option<&str> {
        self.diff.as_deref()
    }
}

/// Execution metrics for a branch.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ExecutionMetrics {
    /// Total execution time in milliseconds.
    pub total_duration_ms: u64,
    /// Number of files processed.
    pub files_processed: usize,
    /// Number of agents executed.
    pub agents_executed: usize,
    /// Peak memory usage in bytes.
    pub peak_memory_bytes: u64,
}

/// Result from a single agent execution.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentResult {
    /// The agent that executed.
    pub agent_id: AgentId,
    /// Whether the execution succeeded.
    pub success: bool,
    /// Output from the agent.
    pub output: Option<String>,
    /// Error message if failed.
    pub error: Option<String>,
    /// Execution duration.
    pub duration_ms: u64,
}

/// Result of branch execution.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BranchResult {
    branch_id: BranchId,
    file_changes: Vec<FileChange>,
    agent_results: Vec<AgentResult>,
    metrics: ExecutionMetrics,
}

impl BranchResult {
    /// Creates a new branch result.
    #[must_use]
    pub fn new(
        branch_id: BranchId,
        file_changes: Vec<FileChange>,
        agent_results: Vec<AgentResult>,
        metrics: ExecutionMetrics,
    ) -> Self {
        Self {
            branch_id,
            file_changes,
            agent_results,
            metrics,
        }
    }

    /// Returns the branch ID.
    #[must_use]
    pub fn branch_id(&self) -> BranchId {
        self.branch_id
    }

    /// Returns the file changes.
    #[must_use]
    pub fn file_changes(&self) -> &[FileChange] {
        &self.file_changes
    }

    /// Returns the agent results.
    #[must_use]
    pub fn agent_results(&self) -> &[AgentResult] {
        &self.agent_results
    }

    /// Returns the metrics.
    #[must_use]
    pub fn metrics(&self) -> &ExecutionMetrics {
        &self.metrics
    }
}

/// Type of merge conflict.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConflictType {
    /// Content conflict - both branches modified the same file.
    Content,
    /// Delete-modify conflict - one branch deleted, other modified.
    DeleteModify,
    /// Add-add conflict - both branches added the same file.
    AddAdd,
    /// Rename conflict - file renamed differently in branches.
    Rename,
}

/// A merge conflict.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Conflict {
    /// Path of the conflicting file.
    pub file_path: PathBuf,
    /// Type of conflict.
    pub conflict_type: ConflictType,
    /// Base content (common ancestor).
    pub base_content: Option<String>,
    /// Content from each conflicting branch.
    pub branch_contents: HashMap<BranchId, String>,
}

/// Result of a merge operation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MergeResult {
    /// Conflicts encountered during merge.
    pub conflicts: Vec<Conflict>,
    /// Files that were successfully resolved.
    pub resolved_files: Vec<PathBuf>,
    /// Strategy used for the merge.
    pub strategy_used: String,
}

impl MergeResult {
    /// Creates a new merge result.
    #[must_use]
    pub fn new(
        conflicts: Vec<Conflict>,
        resolved_files: Vec<PathBuf>,
        strategy_used: impl Into<String>,
    ) -> Self {
        Self {
            conflicts,
            resolved_files,
            strategy_used: strategy_used.into(),
        }
    }

    /// Checks if the merge had any conflicts.
    #[must_use]
    pub fn has_conflicts(&self) -> bool {
        !self.conflicts.is_empty()
    }
}

/// Unique identifier for a merge request in the system.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MergeRequestId(
    /// The underlying numeric identifier (auto-incrementing).
    u64,
);

impl MergeRequestId {
    /// Creates a new `MergeRequestId` from a raw value.
    #[must_use]
    pub const fn new(id: u64) -> Self {
        Self(id)
    }

    /// Returns the inner value.
    #[must_use]
    pub const fn inner(self) -> u64 {
        self.0
    }
}

impl fmt::Display for MergeRequestId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "merge_{}", self.0)
    }
}

/// Merge status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MergeStatus {
    /// Merge request is pending approval.
    Pending,
    /// Merge request has been approved.
    Approved,
    /// Merge request was rejected.
    Rejected,
    /// Merge has been completed.
    Merged,
    /// Merge has conflicts that need resolution.
    Conflict,
}

/// Merge request status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MergeRequestStatus {
    /// Merge request created, awaiting approval.
    Pending,
    /// Merge request has been approved.
    Approved,
    /// Merge is in progress (staging area created).
    InProgress,
    /// Merge has conflicts that need resolution.
    HasConflicts,
    /// All changes staged, ready to commit.
    ReadyToCommit,
    /// Changes have been committed to parent.
    Committed,
    /// Merge was rejected or aborted.
    Rejected,
}

impl MergeRequestStatus {
    /// Checks if the merge can transition to a new status.
    #[must_use]
    pub fn can_transition_to(&self, new_status: Self) -> bool {
        match (self, new_status) {
            (Self::Pending, Self::Approved) => true,
            (Self::Pending, Self::Rejected) => true,
            (Self::Approved, Self::InProgress) => true,
            (Self::InProgress, Self::HasConflicts) => true,
            (Self::InProgress, Self::ReadyToCommit) => true,
            (Self::HasConflicts, Self::InProgress) => true,
            (Self::HasConflicts, Self::ReadyToCommit) => true,
            (Self::ReadyToCommit, Self::Committed) => true,
            (Self::ReadyToCommit, Self::InProgress) => true,
            // Self-transitions are allowed
            (old, new) if std::mem::discriminant(old) == std::mem::discriminant(&new) => true,
            _ => false,
        }
    }
}

/// A file change in the staging area.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StagedChange {
    /// Path of the file.
    pub path: PathBuf,
    /// Type of change.
    pub change_type: ChangeType,
    /// Content hash for verification.
    pub content_hash: Option<String>,
}

/// Enhanced merge request entity with git-like staging workflow.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MergeRequest {
    id: MergeRequestId,
    branch_id: BranchId,
    parent_id: Option<BranchId>,
    strategy: String,
    status: MergeRequestStatus,
    requires_approval: bool,
    approved_by: Option<String>,
    approved_at: Option<i64>,
    created_at: i64,
    /// Session ID for the staging area (merge workspace).
    staging_session_id: Option<String>,
    /// Files staged for merge.
    staged_changes: Vec<StagedChange>,
    /// Detected conflicts.
    conflicts: Vec<Conflict>,
    /// When merge was started.
    started_at: Option<i64>,
    /// When merge was completed.
    completed_at: Option<i64>,
}

impl MergeRequest {
    /// Constructs a new MergeRequest (factory method).
    #[must_use]
    pub fn new(
        id: MergeRequestId,
        branch_id: BranchId,
        parent_id: Option<BranchId>,
        strategy: impl Into<String>,
        requires_approval: bool,
        created_at: i64,
    ) -> Self {
        Self {
            id,
            branch_id,
            parent_id,
            strategy: strategy.into(),
            status: MergeRequestStatus::Pending,
            requires_approval,
            approved_by: None,
            approved_at: None,
            created_at,
            staging_session_id: None,
            staged_changes: Vec::new(),
            conflicts: Vec::new(),
            started_at: None,
            completed_at: None,
        }
    }

    /// Returns the merge request ID.
    #[must_use]
    pub const fn id(&self) -> MergeRequestId {
        self.id
    }

    /// Returns the branch ID being merged.
    #[must_use]
    pub const fn branch_id(&self) -> BranchId {
        self.branch_id
    }

    /// Returns the parent branch ID, if any.
    #[must_use]
    pub const fn parent_id(&self) -> Option<BranchId> {
        self.parent_id
    }

    /// Returns the merge strategy.
    #[must_use]
    pub fn strategy(&self) -> &str {
        &self.strategy
    }

    /// Returns the merge status.
    #[must_use]
    pub const fn status(&self) -> MergeRequestStatus {
        self.status
    }

    /// Returns whether approval is required.
    #[must_use]
    pub const fn requires_approval(&self) -> bool {
        self.requires_approval
    }

    /// Returns the approver, if approved.
    #[must_use]
    pub fn approved_by(&self) -> Option<&str> {
        self.approved_by.as_deref()
    }

    /// Returns the approval timestamp, if approved.
    #[must_use]
    pub const fn approved_at(&self) -> Option<i64> {
        self.approved_at
    }

    /// Returns the creation timestamp.
    #[must_use]
    pub const fn created_at(&self) -> i64 {
        self.created_at
    }

    /// Returns the staging session ID.
    #[must_use]
    pub fn staging_session_id(&self) -> Option<&str> {
        self.staging_session_id.as_deref()
    }

    /// Returns the staged changes.
    #[must_use]
    pub fn staged_changes(&self) -> &[StagedChange] {
        &self.staged_changes
    }

    /// Returns the conflicts.
    #[must_use]
    pub fn conflicts(&self) -> &[Conflict] {
        &self.conflicts
    }

    /// Returns when the merge was started.
    #[must_use]
    pub const fn started_at(&self) -> Option<i64> {
        self.started_at
    }

    /// Returns when the merge was completed.
    #[must_use]
    pub const fn completed_at(&self) -> Option<i64> {
        self.completed_at
    }

    /// Checks if the merge has been approved.
    #[must_use]
    pub const fn is_approved(&self) -> bool {
        matches!(self.status, MergeRequestStatus::Approved)
    }

    /// Checks if the merge has conflicts.
    #[must_use]
    pub fn has_conflicts(&self) -> bool {
        !self.conflicts.is_empty()
    }

    /// Approves the merge request.
    pub fn approve(&mut self, approver: impl Into<String>, timestamp: i64) {
        self.status = MergeRequestStatus::Approved;
        self.approved_by = Some(approver.into());
        self.approved_at = Some(timestamp);
    }

    /// Starts the merge process.
    pub fn start(&mut self, staging_session_id: impl Into<String>, timestamp: i64) {
        self.status = MergeRequestStatus::InProgress;
        self.staging_session_id = Some(staging_session_id.into());
        self.started_at = Some(timestamp);
    }

    /// Updates the staged changes.
    pub fn set_staged_changes(&mut self, changes: Vec<StagedChange>) {
        self.staged_changes = changes;
    }

    /// Updates the conflicts.
    pub fn set_conflicts(&mut self, conflicts: Vec<Conflict>) {
        let has_conflicts = !conflicts.is_empty();
        self.conflicts = conflicts;
        if has_conflicts {
            self.status = MergeRequestStatus::HasConflicts;
        } else {
            self.status = MergeRequestStatus::ReadyToCommit;
        }
    }

    /// Marks conflicts as resolved.
    pub fn mark_conflicts_resolved(&mut self) {
        if self.has_conflicts() {
            self.status = MergeRequestStatus::ReadyToCommit;
        }
    }

    /// Marks the merge as committed.
    pub fn mark_committed(&mut self, timestamp: i64) {
        self.status = MergeRequestStatus::Committed;
        self.completed_at = Some(timestamp);
    }
}

/// The source from which to create a branch.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum BranchSource {
    /// Branch from the base workspace at the given path.
    Base(PathBuf),
    /// Branch from an existing branch.
    Branch(BranchId),
}

/// Strategy for branching task execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BranchingStrategy {
    /// Multiple reviewers for code review from different perspectives.
    MultipleReviewers,
    /// Alternative implementations to compare approaches (A/B testing).
    AlternativeImplementations,
    /// Nested branches for complex refactors with sub-tasks.
    NestedBranches,
}

/// Analyzes task content to determine if branching is needed.
///
/// Returns `Some(BranchingStrategy)` if the task content indicates
/// that branching execution would be beneficial, or `None` if the
/// task should proceed with standard single-path execution.
#[must_use]
pub fn should_use_branching(task: &Task) -> Option<BranchingStrategy> {
    let content = task.content().to_lowercase();

    if content.contains("multiple reviewers")
        || content.contains("security and performance review")
        || content.contains("code review from different perspectives")
    {
        Some(BranchingStrategy::MultipleReviewers)
    } else if content.contains("implement both")
        || content.contains("a/b test")
        || content.contains("compare approaches")
        || content.contains("alternative implementations")
    {
        Some(BranchingStrategy::AlternativeImplementations)
    } else if content.contains("refactor") && content.contains("sub-tasks") {
        Some(BranchingStrategy::NestedBranches)
    } else {
        None
    }
}

/// Unique identifier for a task in the system.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TaskId(
    /// The underlying numeric identifier (auto-incrementing).
    u64,
);

impl TaskId {
    /// Creates a new `TaskId` from a raw value.
    #[must_use]
    pub const fn new(id: u64) -> Self {
        Self(id)
    }

    /// Returns the inner value.
    #[must_use]
    pub const fn inner(self) -> u64 {
        self.0
    }
}

impl fmt::Display for TaskId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "task_{}", self.0)
    }
}

/// Unique identifier for an agent in the mesh.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AgentId(String);

impl AgentId {
    /// Creates a new `AgentId` from a string.
    ///
    /// # Errors
    /// Returns `ValidationError::EmptyAgentId` if the id is empty.
    pub fn new(id: impl Into<String>) -> Result<Self, ValidationError> {
        let id = id.into();
        if id.is_empty() {
            return Err(ValidationError::EmptyAgentId);
        }
        Ok(Self(id))
    }

    /// Returns the inner string reference.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for AgentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Task priority (0-255, higher = more urgent).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Priority(u8);

impl Priority {
    /// Lowest priority value.
    pub const MIN: Self = Self(0);
    /// Highest priority value.
    pub const MAX: Self = Self(255);
    /// Default priority for new tasks.
    pub const DEFAULT: Self = Self(128);

    /// Creates a new Priority from a raw value.
    #[must_use]
    pub const fn new(value: u8) -> Self {
        Self(value)
    }

    /// Returns the inner value.
    #[must_use]
    pub const fn inner(self) -> u8 {
        self.0
    }
}

impl Default for Priority {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// Capabilities that an agent can possess or a task can require.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Capability {
    /// Ability to generate or modify code.
    Coding,
    /// Ability to review code or designs.
    Reviewing,
    /// Ability to reason about system architecture.
    Reasoning,
}

impl fmt::Display for Capability {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Coding => write!(f, "Coding"),
            Self::Reviewing => write!(f, "Reviewing"),
            Self::Reasoning => write!(f, "Reasoning"),
        }
    }
}

/// Task lifecycle status.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskStatus {
    /// Task is waiting to be picked up.
    Pending,
    /// Task is currently analyzing and decomposing requirements.
    Planning,
    /// Task sub-items are being actively worked on.
    Executing,
    /// Task is waiting for sub-tasks to complete.
    Coordinating,
    /// Task is being verified for correctness.
    Verifying,
    /// Task has been assigned to an agent (Legacy/Simple mode).
    Assigned,
    /// Task was completed successfully.
    Completed,
    /// Task failed during execution.
    Failed,
    /// Task is being analyzed for branching strategy.
    AnalyzingForBranch,
    /// Task is executing on multiple branches.
    Branching {
        /// Branch IDs being executed.
        branches: Vec<BranchId>,
        /// Number of branches completed.
        completed: usize,
        /// Total number of branches.
        total: usize,
    },
    /// Task results are being merged.
    Merging {
        /// Branch IDs being merged.
        branches: Vec<BranchId>,
        /// Merge request ID.
        merge_request_id: MergeRequestId,
    },
    /// Merge is pending approval.
    MergePendingApproval {
        /// Branch IDs involved.
        branches: Vec<BranchId>,
        /// Merge request ID.
        merge_request_id: MergeRequestId,
        /// Conflicts requiring resolution.
        conflicts: Vec<Conflict>,
    },
}

impl TaskStatus {
    /// Returns all active statuses that can be represented as simple strings.
    /// These are statuses suitable for database queries.
    #[must_use]
    pub fn active_states() -> Vec<TaskStatus> {
        vec![
            Self::Pending,
            Self::Planning,
            Self::Executing,
            Self::Coordinating,
            Self::Verifying,
            Self::AnalyzingForBranch,
        ]
    }

    /// Parses status from database string representation.
    ///
    /// # Errors
    /// Returns error for unknown status strings.
    pub fn parse(s: &str) -> Result<Self, ParseStatusError> {
        match s.to_lowercase().as_str() {
            "pending" => Ok(Self::Pending),
            "planning" => Ok(Self::Planning),
            "executing" => Ok(Self::Executing),
            "coordinating" => Ok(Self::Coordinating),
            "verifying" => Ok(Self::Verifying),
            "assigned" => Ok(Self::Assigned),
            "completed" => Ok(Self::Completed),
            "failed" => Ok(Self::Failed),
            _ => Err(ParseStatusError(s.to_string())),
        }
    }

    /// Returns the database-compatible string representation.
    ///
    /// Returns `None` for complex variants that require JSON serialization.
    #[must_use]
    pub const fn as_str(&self) -> Option<&'static str> {
        match self {
            Self::Pending => Some("pending"),
            Self::Planning => Some("planning"),
            Self::Executing => Some("executing"),
            Self::Coordinating => Some("coordinating"),
            Self::Verifying => Some("verifying"),
            Self::Assigned => Some("assigned"),
            Self::Completed => Some("completed"),
            Self::Failed => Some("failed"),
            Self::AnalyzingForBranch => Some("analyzing_for_branch"),
            // Complex variants with data should be serialized as JSON
            Self::Branching { .. } => None,
            Self::Merging { .. } => None,
            Self::MergePendingApproval { .. } => None,
        }
    }

    /// Returns a list of all statuses considered "active" (managed by supervisor).
    #[must_use]
    pub fn is_active(&self) -> bool {
        matches!(
            self,
            Self::Pending
                | Self::Planning
                | Self::Executing
                | Self::Coordinating
                | Self::Verifying
                | Self::AnalyzingForBranch
                | Self::Branching { .. }
                | Self::Merging { .. }
                | Self::MergePendingApproval { .. }
        )
    }
}

/// Error when parsing an unknown status string.
#[derive(Debug, Clone)]
pub struct ParseStatusError(pub String);

impl fmt::Display for ParseStatusError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Unknown task status: '{}'", self.0)
    }
}

impl std::error::Error for ParseStatusError {}

/// Immutable task entity representing a unit of work.
#[derive(Debug, Clone)]
pub struct Task {
    id: TaskId,
    content: String,
    priority: Priority,
    status: TaskStatus,
    parent_id: Option<TaskId>,
    assigned_agent: Option<AgentId>,
    required_capabilities: HashSet<Capability>,
}

impl Task {
    /// Constructs a new Task (factory method).
    ///
    /// # Errors
    /// Returns `ValidationError::EmptyTaskContent` if content is empty.
    pub fn new(
        id: TaskId,
        content: String,
        priority: Priority,
        status: TaskStatus,
        parent_id: Option<TaskId>,
        assigned_agent: Option<AgentId>,
        required_capabilities: HashSet<Capability>,
    ) -> Result<Self, ValidationError> {
        if content.is_empty() {
            return Err(ValidationError::EmptyTaskContent);
        }
        Ok(Self {
            id,
            content,
            priority,
            status,
            parent_id,
            assigned_agent,
            required_capabilities,
        })
    }

    /// Returns the task ID.
    #[must_use]
    pub const fn id(&self) -> TaskId {
        self.id
    }

    /// Returns the task content/description.
    #[must_use]
    pub fn content(&self) -> &str {
        &self.content
    }

    /// Returns the task priority.
    #[must_use]
    pub const fn priority(&self) -> Priority {
        self.priority
    }

    /// Returns the current task status.
    #[must_use]
    pub fn status(&self) -> TaskStatus {
        self.status.clone()
    }

    /// Returns the parent task ID, if any.
    #[must_use]
    pub const fn parent_id(&self) -> Option<TaskId> {
        self.parent_id
    }

    /// Returns the assigned agent, if any.
    #[must_use]
    pub fn assigned_agent(&self) -> Option<&AgentId> {
        self.assigned_agent.as_ref()
    }

    /// Returns the capabilities required to perform this task.
    #[must_use]
    pub fn required_capabilities(&self) -> &HashSet<Capability> {
        &self.required_capabilities
    }

    /// Checks if this task is ready for dispatch (Pending).
    #[must_use]
    pub const fn is_pending(&self) -> bool {
        matches!(self.status, TaskStatus::Pending)
    }

    /// Checks if this task is active (managed by supervisor).
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.status.is_active()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_id_display() {
        let id = TaskId::new(42);
        assert_eq!(id.to_string(), "task_42");
    }

    #[test]
    fn agent_id_as_str() {
        let agent = AgentId::new("coder").unwrap();
        assert_eq!(agent.as_str(), "coder");
    }

    #[test]
    fn agent_id_rejects_empty() {
        let err = AgentId::new("").unwrap_err();
        assert!(matches!(err, ValidationError::EmptyAgentId));
    }

    #[test]
    fn priority_ordering() {
        assert!(Priority::MAX > Priority::MIN);
        assert!(Priority::new(100) < Priority::new(200));
    }

    #[test]
    fn task_status_parse_valid() {
        assert_eq!(TaskStatus::parse("pending").unwrap(), TaskStatus::Pending);
        assert_eq!(TaskStatus::parse("ASSIGNED").unwrap(), TaskStatus::Assigned);
        assert_eq!(
            TaskStatus::parse("Completed").unwrap(),
            TaskStatus::Completed
        );
    }

    #[test]
    fn task_status_parse_invalid() {
        let err = TaskStatus::parse("unknown").unwrap_err();
        assert!(err.to_string().contains("unknown"));
    }

    #[test]
    fn task_accessors() {
        let task = Task::new(
            TaskId::new(1),
            "Fix bug".to_string(),
            Priority::DEFAULT,
            TaskStatus::Pending,
            None,
            None,
            HashSet::new(),
        )
        .unwrap();

        assert_eq!(task.id().inner(), 1);
        assert_eq!(task.content(), "Fix bug");
        assert!(task.is_pending());
        assert!(task.assigned_agent().is_none());
    }

    #[test]
    fn task_rejects_empty_content() {
        let err = Task::new(
            TaskId::new(1),
            "".to_string(),
            Priority::DEFAULT,
            TaskStatus::Pending,
            None,
            None,
            HashSet::new(),
        )
        .unwrap_err();

        assert!(matches!(err, ValidationError::EmptyTaskContent));
    }
}
