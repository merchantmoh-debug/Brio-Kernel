//! Task domain - Task entities and status
//!
//! This module defines the Task entity and TaskStatus enum.

use crate::domain::ids::{AgentId, BranchId, Priority, TaskId};
use crate::domain::{Conflict, ParseStatusError, ValidationError};
use crate::merge::MergeId;
use std::collections::HashSet;

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
        merge_request_id: MergeId,
    },
    /// Merge is pending approval.
    MergePendingApproval {
        /// Branch IDs involved.
        branches: Vec<BranchId>,
        /// Merge request ID.
        merge_request_id: MergeId,
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
            Self::Branching { .. } | Self::Merging { .. } | Self::MergePendingApproval { .. } => {
                None
            }
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

/// Immutable task entity representing a unit of work.
#[derive(Debug, Clone)]
pub struct Task {
    id: TaskId,
    content: String,
    priority: Priority,
    status: TaskStatus,
    parent_id: Option<TaskId>,
    assigned_agent: Option<AgentId>,
    required_capabilities: HashSet<super::strategy::Capability>,
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
        required_capabilities: HashSet<super::strategy::Capability>,
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
    pub fn required_capabilities(&self) -> &HashSet<super::strategy::Capability> {
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
