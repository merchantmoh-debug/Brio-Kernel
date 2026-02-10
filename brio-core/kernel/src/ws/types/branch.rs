//! Branch lifecycle types for WebSocket broadcasting.

use serde::{Deserialize, Serialize};
use std::fmt;

use super::events::{ChangeType, ConflictType, EventMetadata, ExecutionStrategy, MergeStrategy};

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

    /// Calculates the percentage complete (0.0 to 100.0) for `ExecutionProgress` events.
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
