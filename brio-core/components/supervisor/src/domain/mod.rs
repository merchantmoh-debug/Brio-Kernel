//! Domain Layer - Value Objects and Entities
//!
//! This module defines the core domain types for the Supervisor.
//! All types are explicit, self-documenting, and follow the principle of
//! making invalid states unrepresentable.
//!
//! The domain module is organized into submodules:
//! - `ids`: Strongly-typed identifiers (`BranchId`, `TaskId`, `AgentId`, etc.)
//! - `errors`: Error types for validation and parsing failures
//! - `branch`: Branch entities, status, and configuration
//! - `task`: Task entities, status, and branching strategy
//! - `merge`: Merge operations, conflicts, and results

// Re-export all public items from submodules
pub use crate::merge::MergeId;
pub use branch::{
    AgentAssignment, BranchConfig, BranchRecord, BranchStatus, ExecutionStrategy,
    MAX_BRANCH_NAME_LEN, MAX_CONCURRENT_BRANCHES, MIN_BRANCH_NAME_LEN,
};
pub use errors::{BranchValidationError, ParseStatusError, ValidationError};
pub use ids::{AgentId, BranchId, Priority, TaskId};
pub use merge::{
    AgentResult, BranchResult, ChangeType, Conflict, ConflictType, ExecutionMetrics, FileChange,
    MergeRequest, MergeRequestStatus, MergeResult, MergeStatus, StagedChange,
};
pub use task::{
    BranchSource, BranchingStrategy, Capability, Task, TaskStatus, should_use_branching,
};

// Declare submodules
mod branch;
mod errors;
mod ids;
pub mod merge;
mod task;

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

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
            String::new(),
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
