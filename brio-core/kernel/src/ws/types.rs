//! Domain types for WebSocket broadcasting.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;
use thiserror::Error;
use uuid::Uuid;

/// Type of change made to a file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeType {
    /// File was added.
    Added,
    /// File was modified.
    Modified,
    /// File was deleted.
    Deleted,
}

impl fmt::Display for ChangeType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Added => write!(f, "added"),
            Self::Modified => write!(f, "modified"),
            Self::Deleted => write!(f, "deleted"),
        }
    }
}

/// Type of merge conflict.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConflictType {
    /// Content conflict in the file.
    Content,
    /// File was deleted in one branch and modified in another.
    DeleteModify,
    /// File was added in both branches with different content.
    AddAdd,
    /// File was renamed differently in both branches.
    RenameRename,
}

impl fmt::Display for ConflictType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Content => write!(f, "content"),
            Self::DeleteModify => write!(f, "delete_modify"),
            Self::AddAdd => write!(f, "add_add"),
            Self::RenameRename => write!(f, "rename_rename"),
        }
    }
}

/// Type of operation being performed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperationType {
    /// Branch execution operation.
    BranchExecution,
    /// Merge operation.
    Merge,
    /// Rollback operation.
    Rollback,
    /// Sync operation.
    Sync,
}

impl fmt::Display for OperationType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BranchExecution => write!(f, "branch_execution"),
            Self::Merge => write!(f, "merge"),
            Self::Rollback => write!(f, "rollback"),
            Self::Sync => write!(f, "sync"),
        }
    }
}

/// Strategy for executing agents on a branch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionStrategy {
    /// Execute agents sequentially.
    Sequential,
    /// Execute agents in parallel.
    Parallel,
}

impl fmt::Display for ExecutionStrategy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Sequential => write!(f, "sequential"),
            Self::Parallel => write!(f, "parallel"),
        }
    }
}

/// Strategy for merging branches.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MergeStrategy {
    /// Fast-forward merge if possible.
    FastForward,
    /// Create a merge commit.
    MergeCommit,
    /// Squash commits into one.
    Squash,
    /// Rebase and merge.
    Rebase,
}

impl fmt::Display for MergeStrategy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FastForward => write!(f, "fast_forward"),
            Self::MergeCommit => write!(f, "merge_commit"),
            Self::Squash => write!(f, "squash"),
            Self::Rebase => write!(f, "rebase"),
        }
    }
}

/// Unique identifier for a branch.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BranchId(String);

impl BranchId {
    /// Creates a new branch ID from a string.
    ///
    /// # Arguments
    ///
    /// * `id` - The string identifier for the branch.
    #[must_use]
    pub fn new(id: String) -> Self {
        Self(id)
    }

    /// Returns the underlying string ID.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for BranchId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Metadata common to all branch events.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventMetadata {
    /// Unique identifier for this event.
    pub event_id: String,
    /// Timestamp when the event occurred.
    pub timestamp: DateTime<Utc>,
}

impl EventMetadata {
    /// Creates new event metadata with the current timestamp.
    #[must_use]
    pub fn new() -> Self {
        Self {
            event_id: Uuid::new_v4().to_string(),
            timestamp: Utc::now(),
        }
    }
}

impl Default for EventMetadata {
    fn default() -> Self {
        Self::new()
    }
}

/// Unique identifier for a WebSocket client.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ClientId(Uuid);

impl ClientId {
    /// Generates a new unique client ID.
    #[must_use]
    pub fn generate() -> Self {
        Self(Uuid::new_v4())
    }

    /// Returns the underlying UUID.
    #[must_use]
    pub fn as_uuid(&self) -> Uuid {
        self.0
    }
}

impl fmt::Display for ClientId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A WebSocket patch message containing a JSON Patch.
#[derive(Debug, Clone)]
pub struct WsPatch {
    inner: json_patch::Patch,
}

impl WsPatch {
    /// Creates a new WebSocket patch.
    ///
    /// # Arguments
    ///
    /// * `patch` - The JSON Patch to wrap.
    #[must_use]
    pub fn new(patch: json_patch::Patch) -> Self {
        Self { inner: patch }
    }

    /// Returns a reference to the underlying JSON Patch.
    #[must_use]
    pub fn inner(&self) -> &json_patch::Patch {
        &self.inner
    }

    /// Serializes the patch to a JSON string.
    ///
    /// # Errors
    ///
    /// Returns an error if the patch cannot be serialized to JSON.
    pub fn to_json(&self) -> Result<String, WsError> {
        serde_json::to_string(&self.inner).map_err(WsError::Serialization)
    }
}

/// Messages that can be broadcast to WebSocket clients.
#[derive(Debug, Clone)]
pub enum BroadcastMessage {
    /// A JSON Patch message.
    Patch(Box<WsPatch>),
    /// Server shutdown signal.
    Shutdown,
    /// A structured WebSocket message.
    Message(WsMessage),
}

/// Summary of branch result (for WebSocket)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchResultSummary {
    /// List of file changes made by the branch.
    file_changes: Vec<FileChangeSummary>,
    /// Number of agents that executed on the branch.
    agent_count: usize,
    /// Total execution time in seconds.
    execution_time_secs: u64,
}

impl BranchResultSummary {
    /// Creates a new branch result summary.
    ///
    /// # Arguments
    ///
    /// * `file_changes` - List of file changes made by the branch.
    /// * `agent_count` - Number of agents that executed on the branch.
    /// * `execution_time_secs` - Total execution time in seconds.
    #[must_use]
    pub fn new(
        file_changes: Vec<FileChangeSummary>,
        agent_count: usize,
        execution_time_secs: u64,
    ) -> Self {
        Self {
            file_changes,
            agent_count,
            execution_time_secs,
        }
    }

    /// Returns the list of file changes made by the branch.
    #[must_use]
    pub fn file_changes(&self) -> &[FileChangeSummary] {
        &self.file_changes
    }

    /// Returns the number of agents that executed on the branch.
    #[must_use]
    pub fn agent_count(&self) -> usize {
        self.agent_count
    }

    /// Returns the total execution time in seconds.
    #[must_use]
    pub fn execution_time_secs(&self) -> u64 {
        self.execution_time_secs
    }
}

/// Summary of a single file change.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileChangeSummary {
    /// Path of the changed file.
    path: String,
    /// Type of change.
    change_type: ChangeType,
    /// Number of lines changed (optional).
    lines_changed: Option<usize>,
}

impl FileChangeSummary {
    /// Creates a new file change summary.
    ///
    /// # Arguments
    ///
    /// * `path` - Path of the changed file.
    /// * `change_type` - Type of change made.
    /// * `lines_changed` - Optional number of lines changed.
    #[must_use]
    pub fn new(path: String, change_type: ChangeType, lines_changed: Option<usize>) -> Self {
        Self {
            path,
            change_type,
            lines_changed,
        }
    }

    /// Returns the path of the changed file.
    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }

    /// Returns the type of change.
    #[must_use]
    pub fn change_type(&self) -> ChangeType {
        self.change_type
    }

    /// Returns the number of lines changed, if available.
    #[must_use]
    pub fn lines_changed(&self) -> Option<usize> {
        self.lines_changed
    }
}

/// Summary of a merge conflict.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConflictSummary {
    /// Path of the conflicting file.
    file_path: String,
    /// Type of conflict.
    conflict_type: ConflictType,
    /// IDs of branches involved in the conflict.
    branches_involved: Vec<BranchId>,
}

impl ConflictSummary {
    /// Creates a new conflict summary.
    ///
    /// # Arguments
    ///
    /// * `file_path` - Path of the conflicting file.
    /// * `conflict_type` - Type of conflict.
    /// * `branches_involved` - IDs of branches involved in the conflict.
    #[must_use]
    pub fn new(
        file_path: String,
        conflict_type: ConflictType,
        branches_involved: Vec<BranchId>,
    ) -> Self {
        Self {
            file_path,
            conflict_type,
            branches_involved,
        }
    }

    /// Returns the path of the conflicting file.
    #[must_use]
    pub fn file_path(&self) -> &str {
        &self.file_path
    }

    /// Returns the type of conflict.
    #[must_use]
    pub fn conflict_type(&self) -> ConflictType {
        self.conflict_type
    }

    /// Returns the IDs of branches involved in the conflict.
    #[must_use]
    pub fn branches_involved(&self) -> &[BranchId] {
        &self.branches_involved
    }
}

/// Errors that can occur when creating a progress update.
#[derive(Debug, Error)]
pub enum ProgressUpdateError {
    /// Total items must be greater than zero.
    #[error("total_items must be greater than zero")]
    InvalidTotalItems,
    /// Completed items cannot exceed total items.
    #[error("completed_items cannot exceed total_items")]
    InvalidCompletedItems,
}

/// Progress update for long-running operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgressUpdate {
    /// Unique identifier for the operation.
    operation_id: String,
    /// Type of operation.
    operation_type: OperationType,
    /// Total number of items to process.
    total_items: usize,
    /// Number of items completed so far.
    completed_items: usize,
    /// Current item being processed (if any).
    current_item: Option<String>,
    /// Timestamp of the update.
    timestamp: DateTime<Utc>,
}

impl ProgressUpdate {
    /// Creates a new progress update.
    ///
    /// # Arguments
    ///
    /// * `operation_id` - Unique identifier for the operation.
    /// * `operation_type` - Type of operation being performed.
    /// * `total_items` - Total number of items to process (must be > 0).
    /// * `completed_items` - Number of items completed so far.
    /// * `current_item` - Current item being processed (optional).
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// * `total_items` is zero
    /// * `completed_items` exceeds `total_items`
    pub fn new(
        operation_id: String,
        operation_type: OperationType,
        total_items: usize,
        completed_items: usize,
        current_item: Option<String>,
    ) -> Result<Self, ProgressUpdateError> {
        if total_items == 0 {
            return Err(ProgressUpdateError::InvalidTotalItems);
        }
        if completed_items > total_items {
            return Err(ProgressUpdateError::InvalidCompletedItems);
        }
        Ok(Self {
            operation_id,
            operation_type,
            total_items,
            completed_items,
            current_item,
            timestamp: Utc::now(),
        })
    }

    /// Returns the operation ID.
    #[must_use]
    pub fn operation_id(&self) -> &str {
        &self.operation_id
    }

    /// Returns the operation type.
    #[must_use]
    pub fn operation_type(&self) -> OperationType {
        self.operation_type
    }

    /// Returns the total number of items.
    #[must_use]
    pub fn total_items(&self) -> usize {
        self.total_items
    }

    /// Returns the number of completed items.
    #[must_use]
    pub fn completed_items(&self) -> usize {
        self.completed_items
    }

    /// Returns the current item being processed, if any.
    #[must_use]
    pub fn current_item(&self) -> Option<&str> {
        self.current_item.as_deref()
    }

    /// Returns the timestamp of the update.
    #[must_use]
    pub fn timestamp(&self) -> DateTime<Utc> {
        self.timestamp
    }

    /// Calculates the percentage complete (0.0 to 100.0).
    #[must_use]
    pub fn percent_complete(&self) -> f32 {
        (self.completed_items as f32 / self.total_items as f32) * 100.0
    }
}

/// Events related to branch lifecycle
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BranchEvent {
    /// New branch created
    Created {
        /// Unique identifier for the branch.
        branch_id: BranchId,
        /// Parent branch ID (if this is a child branch).
        parent_id: Option<BranchId>,
        /// Name of the branch.
        name: String,
        /// VFS session ID associated with this branch.
        session_id: String,
        /// Common event metadata.
        #[serde(flatten)]
        metadata: EventMetadata,
    },

    /// Branch started executing
    ExecutionStarted {
        /// ID of the branch being executed.
        branch_id: BranchId,
        /// List of agent IDs assigned to this branch.
        agents: Vec<String>,
        /// Execution strategy being used.
        execution_strategy: ExecutionStrategy,
        /// Common event metadata.
        #[serde(flatten)]
        metadata: EventMetadata,
    },

    /// Progress update during execution
    ExecutionProgress {
        /// ID of the branch being executed.
        branch_id: BranchId,
        /// Total number of agents assigned to this branch.
        total_agents: usize,
        /// Number of agents that have completed.
        completed_agents: usize,
        /// ID of the agent currently executing (if any).
        current_agent: Option<String>,
        /// Common event metadata.
        #[serde(flatten)]
        metadata: EventMetadata,
    },

    /// Agent completed on branch
    AgentCompleted {
        /// ID of the branch the agent executed on.
        branch_id: BranchId,
        /// ID of the agent that completed.
        agent_id: String,
        /// Brief summary of the agent's results.
        result_summary: String,
        /// Common event metadata.
        #[serde(flatten)]
        metadata: EventMetadata,
    },

    /// Branch execution completed
    ExecutionCompleted {
        /// ID of the completed branch.
        branch_id: BranchId,
        /// Summary of the branch execution results.
        result: BranchResultSummary,
        /// Number of files changed by the branch.
        file_changes_count: usize,
        /// Common event metadata.
        #[serde(flatten)]
        metadata: EventMetadata,
    },

    /// Branch execution failed
    ExecutionFailed {
        /// ID of the branch that failed.
        branch_id: BranchId,
        /// Error message describing what went wrong.
        error: String,
        /// ID of the agent that failed (if known).
        failed_agent: Option<String>,
        /// Common event metadata.
        #[serde(flatten)]
        metadata: EventMetadata,
    },

    /// Branch is being merged
    MergeStarted {
        /// ID of the branch being merged.
        branch_id: BranchId,
        /// Merge strategy being used.
        strategy: MergeStrategy,
        /// Whether manual approval is required.
        requires_approval: bool,
        /// Common event metadata.
        #[serde(flatten)]
        metadata: EventMetadata,
    },

    /// Merge completed successfully
    MergeCompleted {
        /// ID of the branch that was merged.
        branch_id: BranchId,
        /// Strategy that was actually used for the merge.
        strategy_used: MergeStrategy,
        /// Number of files changed by the merge.
        files_changed: usize,
        /// Common event metadata.
        #[serde(flatten)]
        metadata: EventMetadata,
    },

    /// Merge has conflicts requiring resolution
    MergeConflict {
        /// ID of the branch with conflicts.
        branch_id: BranchId,
        /// List of conflicts that need resolution.
        conflicts: Vec<ConflictSummary>,
        /// ID of the merge request.
        merge_request_id: String,
        /// Common event metadata.
        #[serde(flatten)]
        metadata: EventMetadata,
    },

    /// Branch was rolled back/aborted
    RolledBack {
        /// ID of the branch that was rolled back.
        branch_id: BranchId,
        /// Reason for the rollback.
        reason: String,
        /// Common event metadata.
        #[serde(flatten)]
        metadata: EventMetadata,
    },
}

impl BranchEvent {
    /// Returns the branch ID associated with this event.
    #[must_use]
    pub fn branch_id(&self) -> &BranchId {
        match self {
            Self::Created { branch_id, .. } => branch_id,
            Self::ExecutionStarted { branch_id, .. } => branch_id,
            Self::ExecutionProgress { branch_id, .. } => branch_id,
            Self::AgentCompleted { branch_id, .. } => branch_id,
            Self::ExecutionCompleted { branch_id, .. } => branch_id,
            Self::ExecutionFailed { branch_id, .. } => branch_id,
            Self::MergeStarted { branch_id, .. } => branch_id,
            Self::MergeCompleted { branch_id, .. } => branch_id,
            Self::MergeConflict { branch_id, .. } => branch_id,
            Self::RolledBack { branch_id, .. } => branch_id,
        }
    }

    /// Returns the event metadata.
    #[must_use]
    pub fn metadata(&self) -> &EventMetadata {
        match self {
            Self::Created { metadata, .. } => metadata,
            Self::ExecutionStarted { metadata, .. } => metadata,
            Self::ExecutionProgress { metadata, .. } => metadata,
            Self::AgentCompleted { metadata, .. } => metadata,
            Self::ExecutionCompleted { metadata, .. } => metadata,
            Self::ExecutionFailed { metadata, .. } => metadata,
            Self::MergeStarted { metadata, .. } => metadata,
            Self::MergeCompleted { metadata, .. } => metadata,
            Self::MergeConflict { metadata, .. } => metadata,
            Self::RolledBack { metadata, .. } => metadata,
        }
    }
}

impl BranchEvent {
    /// Calculates the percentage complete (0.0 to 100.0) for ExecutionProgress events.
    ///
    /// # Panics
    ///
    /// Panics if called on a non-ExecutionProgress event.
    #[must_use]
    pub fn percent_complete(&self) -> f32 {
        match self {
            Self::ExecutionProgress {
                completed_agents,
                total_agents,
                ..
            } => (*completed_agents as f32 / *total_agents as f32) * 100.0,
            _ => panic!("percent_complete called on non-ExecutionProgress event"),
        }
    }
}

/// Merge request events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MergeRequestEvent {
    /// New merge request created
    Created {
        /// Unique identifier for the merge request.
        merge_request_id: String,
        /// ID of the branch to be merged.
        branch_id: BranchId,
        /// Merge strategy proposed.
        strategy: MergeStrategy,
        /// Whether manual approval is required.
        requires_approval: bool,
        /// Common event metadata.
        #[serde(flatten)]
        metadata: EventMetadata,
    },

    /// Merge request approved
    Approved {
        /// ID of the approved merge request.
        merge_request_id: String,
        /// User or system that approved the merge.
        approver: String,
        /// Common event metadata.
        #[serde(flatten)]
        metadata: EventMetadata,
    },

    /// Merge request rejected
    Rejected {
        /// ID of the rejected merge request.
        merge_request_id: String,
        /// Reason for rejection.
        reason: String,
        /// Common event metadata.
        #[serde(flatten)]
        metadata: EventMetadata,
    },

    /// Merge request completed
    Completed {
        /// ID of the completed merge request.
        merge_request_id: String,
        /// ID of the branch that was merged.
        branch_id: BranchId,
        /// Whether the merge was successful.
        success: bool,
        /// Common event metadata.
        #[serde(flatten)]
        metadata: EventMetadata,
    },
}

/// WebSocket message types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WsMessage {
    /// Branch lifecycle event
    BranchEvent(BranchEvent),
    /// Merge request event
    MergeRequestEvent(MergeRequestEvent),
    /// Progress update for long-running operations
    ProgressUpdate(ProgressUpdate),
}

impl BroadcastMessage {
    /// Converts the message to a WebSocket frame payload.
    ///
    /// # Errors
    ///
    /// Returns an error if a patch message cannot be serialized to JSON.
    pub fn to_frame_payload(&self) -> Result<String, WsError> {
        match self {
            Self::Patch(patch) => patch.to_json(),
            Self::Shutdown => Ok(r#"{"type":"shutdown"}"#.to_string()),
            Self::Message(msg) => serde_json::to_string(msg).map_err(WsError::Serialization),
        }
    }
}

/// Errors that can occur in WebSocket operations.
#[derive(Debug, Error)]
pub enum WsError {
    /// WebSocket connection error.
    #[error("WebSocket connection error: {0}")]
    AxumWs(#[from] axum::Error),

    /// JSON serialization error.
    #[error("Serialization error: {0}")]
    Serialization(#[source] serde_json::Error),

    /// The broadcast channel was closed.
    #[error("Broadcast channel closed")]
    ChannelClosed,

    /// Client disconnected from the connection.
    #[error("Connection closed by client")]
    ClientDisconnected,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn client_id_is_unique() {
        let id1 = ClientId::generate();
        let id2 = ClientId::generate();
        assert_ne!(id1, id2);
    }

    #[test]
    fn client_id_display() {
        let id = ClientId::generate();
        let display = format!("{id}");
        assert!(!display.is_empty());
    }

    #[test]
    fn broadcast_message_shutdown_serializes() -> Result<(), WsError> {
        let msg = BroadcastMessage::Shutdown;
        let payload = msg.to_frame_payload()?;
        assert_eq!(payload, r#"{"type":"shutdown"}"#);
        Ok(())
    }
}
