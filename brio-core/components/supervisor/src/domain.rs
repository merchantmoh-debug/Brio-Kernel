//! Domain Layer - Value Objects and Entities
//!
//! This module defines the core domain types for the Supervisor.
//! All types are explicit, self-documenting, and follow the principle of
//! making invalid states unrepresentable.

use core::fmt;

// =============================================================================
// Value Objects (Type-Safe Wrappers)
// =============================================================================

/// Unique identifier for a task in the system.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TaskId(u64);

impl TaskId {
    /// Creates a new TaskId from a raw value.
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
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AgentId(String);

impl AgentId {
    /// Creates a new AgentId from a string.
    ///
    /// # Panics
    /// Panics if the id is empty (Design by Contract).
    #[must_use]
    pub fn new(id: impl Into<String>) -> Self {
        let id = id.into();
        assert!(!id.is_empty(), "AgentId cannot be empty");
        Self(id)
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
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

// =============================================================================
// Task Status (State Machine)
// =============================================================================

/// Task lifecycle status following strict state transitions:
/// `Pending` → `Assigned` → `Completed` | `Failed`
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskStatus {
    /// Task is waiting to be picked up.
    Pending,
    /// Task is currently analyzing and decomposing requirements.
    Planning,
    /// Task sub-items are being actively worked on.
    Executing,
    /// Task is being verified for correctness.
    Verifying,
    /// Task has been assigned to an agent (Legacy/Simple mode).
    Assigned,
    /// Task was completed successfully.
    Completed,
    /// Task failed during execution.
    Failed,
}

impl TaskStatus {
    /// Parses status from database string representation.
    ///
    /// # Errors
    /// Returns error for unknown status strings.
    pub fn parse(s: &str) -> Result<Self, ParseStatusError> {
        match s.to_lowercase().as_str() {
            "pending" => Ok(Self::Pending),
            "planning" => Ok(Self::Planning),
            "executing" => Ok(Self::Executing),
            "verifying" => Ok(Self::Verifying),
            "assigned" => Ok(Self::Assigned),
            "completed" => Ok(Self::Completed),
            "failed" => Ok(Self::Failed),
            _ => Err(ParseStatusError(s.to_string())),
        }
    }

    /// Returns the database-compatible string representation.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Planning => "planning",
            Self::Executing => "executing",
            Self::Verifying => "verifying",
            Self::Assigned => "assigned",
            Self::Completed => "completed",
            Self::Failed => "failed",
        }
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

// =============================================================================
// Task Entity
// =============================================================================

/// Immutable task entity representing a unit of work.
#[derive(Debug, Clone)]
pub struct Task {
    id: TaskId,
    content: String,
    priority: Priority,
    status: TaskStatus,
    assigned_agent: Option<AgentId>,
}

impl Task {
    /// Constructs a new Task (factory method).
    #[must_use]
    pub fn new(
        id: TaskId,
        content: String,
        priority: Priority,
        status: TaskStatus,
        assigned_agent: Option<AgentId>,
    ) -> Self {
        Self {
            id,
            content,
            priority,
            status,
            assigned_agent,
        }
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
    pub const fn status(&self) -> TaskStatus {
        self.status
    }

    /// Returns the assigned agent, if any.
    #[must_use]
    pub fn assigned_agent(&self) -> Option<&AgentId> {
        self.assigned_agent.as_ref()
    }

    /// Checks if this task is ready for dispatch (Pending).
    #[must_use]
    pub const fn is_pending(&self) -> bool {
        matches!(self.status, TaskStatus::Pending)
    }

    /// Checks if this task is active (managed by supervisor).
    #[must_use]
    pub const fn is_active(&self) -> bool {
        matches!(
            self.status,
            TaskStatus::Pending
                | TaskStatus::Planning
                | TaskStatus::Executing
                | TaskStatus::Verifying
        )
    }
}

// =============================================================================
// Unit Tests
// =============================================================================

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
        let agent = AgentId::new("coder");
        assert_eq!(agent.as_str(), "coder");
    }

    #[test]
    #[should_panic(expected = "AgentId cannot be empty")]
    fn agent_id_rejects_empty() {
        let _ = AgentId::new("");
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
        );

        assert_eq!(task.id().inner(), 1);
        assert_eq!(task.content(), "Fix bug");
        assert!(task.is_pending());
        assert!(task.assigned_agent().is_none());
    }
}
