//! Domain Layer - Value Objects and Entities
//!
//! This module defines the core domain types for the Supervisor.
//! All types are explicit, self-documenting, and follow the principle of
//! making invalid states unrepresentable.

use core::fmt;
use std::collections::HashSet;

/// Error type for domain validation failures.
#[derive(Debug, Clone)]
pub enum ValidationError {
    /// AgentId cannot be empty.
    EmptyAgentId,
    /// Task content cannot be empty.
    EmptyTaskContent,
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyAgentId => write!(f, "AgentId cannot be empty"),
            Self::EmptyTaskContent => write!(f, "Task content cannot be empty"),
        }
    }
}

impl std::error::Error for ValidationError {}

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
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
            "coordinating" => Ok(Self::Coordinating),
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
            Self::Coordinating => "coordinating",
            Self::Verifying => "verifying",
            Self::Assigned => "assigned",
            Self::Completed => "completed",
            Self::Failed => "failed",
        }
    }

    /// Returns a list of all statuses considered "active" (managed by supervisor).
    #[must_use]
    pub const fn active_states() -> &'static [Self] {
        &[
            Self::Pending,
            Self::Planning,
            Self::Executing,
            Self::Coordinating,
            Self::Verifying,
        ]
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
    pub const fn status(&self) -> TaskStatus {
        self.status
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
        TaskStatus::active_states().contains(&self.status)
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
