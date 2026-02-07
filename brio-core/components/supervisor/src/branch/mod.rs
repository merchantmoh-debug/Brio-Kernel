//! Branch Domain Types
//!
//! Core domain types for the branching orchestrator system.
//! All types follow the principle of making invalid states unrepresentable.

pub mod events;
pub mod execution;
pub mod manager;

use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// Re-export domain types that are shared
pub use crate::domain::{
    AgentAssignment, AgentResult, BranchConfig, BranchId, BranchResult, BranchStatus,
    BranchStatusKind, BranchValidationError, ChangeType, Conflict, ConflictType, ExecutionMetrics,
    ExecutionStrategy, FileChange, MergeResult, MAX_BRANCH_NAME_LEN, MAX_CONCURRENT_BRANCHES,
    MIN_BRANCH_NAME_LEN,
};

impl BranchStatus {
    /// Validates that a transition from this status to the target is allowed.
    ///
    /// # Errors
    /// Returns `BranchValidationError::InvalidStatusTransition` if the transition is invalid.
    pub fn validate_transition(&self, target: &Self) -> Result<(), BranchValidationError> {
        let current_kind = BranchStatusKind::from(self);
        let target_kind = BranchStatusKind::from(target);

        let valid = match (self, target) {
            // Pending can transition to Active or Failed
            (Self::Pending, Self::Active) => true,
            (Self::Pending, Self::Failed) => true,
            // Active can transition to Completed, Merging, or Failed
            (Self::Active, Self::Completed) => true,
            (Self::Active, Self::Merging) => true,
            (Self::Active, Self::Failed) => true,
            // Completed can transition to Merging
            (Self::Completed, Self::Merging) => true,
            // Merging can transition to Merged or Failed
            (Self::Merging, Self::Merged) => true,
            (Self::Merging, Self::Failed) => true,
            // Terminal states cannot transition
            (Self::Merged, _) => false,
            (Self::Failed, _) => false,
            // All other transitions are invalid
            _ => false,
        };

        if valid {
            Ok(())
        } else {
            Err(BranchValidationError::InvalidStatusTransition {
                from: current_kind,
                to: target_kind,
            })
        }
    }
}

/// The source from which to create a branch.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum BranchSource {
    /// Branch from the base workspace at the given path.
    Base(PathBuf),
    /// Branch from an existing branch.
    Branch(BranchId),
    /// Branch from a session snapshot.
    Snapshot(SessionSnapshot),
}

/// A snapshot of a VFS session for branching.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionSnapshot {
    session_id: String,
    timestamp: DateTime<Utc>,
    description: Option<String>,
}

impl SessionSnapshot {
    /// Creates a new session snapshot.
    ///
    /// # Errors
    /// Returns `BranchValidationError::EmptySessionId` if session_id is empty.
    pub fn new(
        session_id: impl Into<String>,
        timestamp: DateTime<Utc>,
        description: Option<String>,
    ) -> Result<Self, BranchValidationError> {
        let session_id = session_id.into();
        if session_id.is_empty() {
            return Err(BranchValidationError::EmptySessionId);
        }
        Ok(Self {
            session_id,
            timestamp,
            description,
        })
    }

    /// Returns the session identifier.
    #[must_use]
    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    /// Returns the timestamp when the snapshot was created.
    #[must_use]
    pub const fn timestamp(&self) -> DateTime<Utc> {
        self.timestamp
    }

    /// Returns an optional description of the snapshot.
    #[must_use]
    pub fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }
}

/// The main Branch entity.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Branch {
    id: BranchId,
    parent_id: Option<BranchId>,
    session_id: String,
    name: String,
    status: BranchStatus,
    children: Vec<BranchId>,
    config: BranchConfig,
    created_at: DateTime<Utc>,
    completed_at: Option<DateTime<Utc>>,
    /// Execution result data (stored separately from status).
    execution_result: Option<BranchResult>,
    /// Merge result data (stored separately from status).
    merge_result: Option<MergeResult>,
    /// Failure reason (stored separately from status).
    failure_reason: Option<String>,
}

impl Branch {
    /// Creates a new Branch with validation.
    ///
    /// # Errors
    /// Returns `BranchValidationError` if validation fails.
    #[must_use]
    pub fn new(
        id: BranchId,
        parent_id: Option<BranchId>,
        session_id: impl Into<String>,
        name: impl Into<String>,
        config: BranchConfig,
    ) -> Result<Self, BranchValidationError> {
        let session_id = session_id.into();
        let name = name.into();

        if session_id.is_empty() {
            return Err(BranchValidationError::EmptySessionId);
        }

        let name_len = name.len();
        if name_len < MIN_BRANCH_NAME_LEN || name_len > MAX_BRANCH_NAME_LEN {
            return Err(BranchValidationError::InvalidNameLength {
                len: name_len,
                min: MIN_BRANCH_NAME_LEN,
                max: MAX_BRANCH_NAME_LEN,
            });
        }

        Ok(Self {
            id,
            parent_id,
            session_id,
            name,
            status: BranchStatus::Pending,
            children: Vec::new(),
            config,
            created_at: Utc::now(),
            completed_at: None,
            execution_result: None,
            merge_result: None,
            failure_reason: None,
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

    /// Returns the VFS session ID.
    #[must_use]
    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    /// Returns the branch name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the current status.
    #[must_use]
    pub fn status(&self) -> &BranchStatus {
        &self.status
    }

    /// Returns the child branch IDs.
    #[must_use]
    pub fn children(&self) -> &[BranchId] {
        &self.children
    }

    /// Returns the branch configuration.
    #[must_use]
    pub fn config(&self) -> &BranchConfig {
        &self.config
    }

    /// Returns the creation timestamp.
    #[must_use]
    pub const fn created_at(&self) -> DateTime<Utc> {
        self.created_at
    }

    /// Returns the completion timestamp, if completed.
    #[must_use]
    pub const fn completed_at(&self) -> Option<DateTime<Utc>> {
        self.completed_at
    }

    /// Checks if the branch is in a terminal state.
    #[must_use]
    pub fn is_terminal(&self) -> bool {
        self.status.is_terminal()
    }

    /// Checks if the branch is active.
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.status.is_active()
    }

    /// Adds a child branch ID.
    pub fn add_child(&mut self, child_id: BranchId) {
        if !self.children.contains(&child_id) {
            self.children.push(child_id);
        }
    }

    /// Removes a child branch ID.
    pub fn remove_child(&mut self, child_id: BranchId) {
        self.children.retain(|&id| id != child_id);
    }

    /// Updates the branch status with transition validation.
    ///
    /// # Errors
    /// Returns `BranchValidationError::InvalidStatusTransition` if the transition is invalid.
    pub fn update_status(&mut self, new_status: BranchStatus) -> Result<(), BranchValidationError> {
        self.status.validate_transition(&new_status)?;

        // Update completed_at if transitioning to terminal state
        if new_status.is_terminal() && self.completed_at.is_none() {
            self.completed_at = Some(Utc::now());
        }

        self.status = new_status;
        Ok(())
    }

    /// Starts branch execution.
    ///
    /// # Errors
    /// Returns `BranchValidationError` if the branch cannot be started.
    pub fn start_execution(&mut self) -> Result<(), BranchValidationError> {
        self.update_status(BranchStatus::Active)
    }

    /// Marks the branch as completed.
    ///
    /// # Errors
    /// Returns `BranchValidationError` if the branch cannot be completed.
    pub fn complete(&mut self, result: BranchResult) -> Result<(), BranchValidationError> {
        self.execution_result = Some(result);
        self.update_status(BranchStatus::Completed)
    }

    /// Starts merging the branch.
    ///
    /// # Errors
    /// Returns `BranchValidationError` if the branch cannot be merged.
    pub fn start_merge(&mut self) -> Result<(), BranchValidationError> {
        self.update_status(BranchStatus::Merging)
    }

    /// Marks the branch as merged.
    ///
    /// # Errors
    /// Returns `BranchValidationError` if the branch cannot be marked as merged.
    pub fn mark_merged(&mut self, result: MergeResult) -> Result<(), BranchValidationError> {
        self.merge_result = Some(result);
        self.update_status(BranchStatus::Merged)
    }

    /// Marks the branch as failed.
    ///
    /// # Errors
    /// Returns `BranchValidationError` if the branch cannot be marked as failed.
    pub fn fail(&mut self, reason: impl Into<String>) -> Result<(), BranchValidationError> {
        self.failure_reason = Some(reason.into());
        self.update_status(BranchStatus::Failed)
    }

    /// Returns the execution result, if completed.
    #[must_use]
    pub fn execution_result(&self) -> Option<&BranchResult> {
        self.execution_result.as_ref()
    }

    /// Returns the merge result, if merged.
    #[must_use]
    pub fn merge_result(&self) -> Option<&MergeResult> {
        self.merge_result.as_ref()
    }

    /// Returns the failure reason, if failed.
    #[must_use]
    pub fn failure_reason(&self) -> Option<&str> {
        self.failure_reason.as_deref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    mod branch_id_tests {
        use super::*;

        #[test]
        fn branch_id_new_creates_unique() {
            let id1 = BranchId::new();
            let id2 = BranchId::new();
            assert_ne!(id1, id2);
        }

        #[test]
        fn branch_id_display() {
            let id = BranchId::from_uuid(Uuid::nil());
            assert_eq!(id.to_string(), "00000000-0000-0000-0000-000000000000");
        }

        #[test]
        fn branch_id_inner() {
            let uuid = Uuid::new_v4();
            let id = BranchId::from_uuid(uuid);
            assert_eq!(id.inner(), uuid);
        }

        #[test]
        fn branch_id_default() {
            let id: BranchId = Default::default();
            // Just verify it doesn't panic and creates a valid UUID
            assert!(!id.inner().to_string().is_empty());
        }
    }

    mod branch_status_tests {
        use super::*;

        #[test]
        fn status_is_terminal_for_completed() {
            assert!(BranchStatus::Completed.is_terminal());
        }

        #[test]
        fn status_is_terminal_for_merged() {
            assert!(BranchStatus::Merged.is_terminal());
        }

        #[test]
        fn status_is_terminal_for_failed() {
            assert!(BranchStatus::Failed.is_terminal());
        }

        #[test]
        fn status_is_not_terminal_for_pending() {
            assert!(!BranchStatus::Pending.is_terminal());
        }

        #[test]
        fn status_is_not_terminal_for_active() {
            assert!(!BranchStatus::Active.is_terminal());
        }

        #[test]
        fn status_is_active_for_active() {
            assert!(BranchStatus::Active.is_active());
        }

        #[test]
        fn status_is_active_for_merging() {
            assert!(BranchStatus::Merging.is_active());
        }

        #[test]
        fn status_validate_transition_pending_to_active() {
            let pending = BranchStatus::Pending;
            let active = BranchStatus::Active;
            assert!(pending.validate_transition(&active).is_ok());
        }

        #[test]
        fn status_validate_transition_pending_to_completed_fails() {
            let pending = BranchStatus::Pending;
            let completed = BranchStatus::Completed;
            assert!(pending.validate_transition(&completed).is_err());
        }

        #[test]
        fn status_validate_transition_terminal_to_anything_fails() {
            let merged = BranchStatus::Merged;
            let active = BranchStatus::Active;
            assert!(merged.validate_transition(&active).is_err());
        }

        #[test]
        fn status_validate_all_valid_transitions() {
            // Pending -> Active (valid)
            assert!(BranchStatus::Pending.validate_transition(&BranchStatus::Active).is_ok());
            // Pending -> Failed (valid)
            assert!(BranchStatus::Pending.validate_transition(&BranchStatus::Failed).is_ok());
            // Active -> Completed (valid)
            assert!(BranchStatus::Active.validate_transition(&BranchStatus::Completed).is_ok());
            // Active -> Merging (valid)
            assert!(BranchStatus::Active.validate_transition(&BranchStatus::Merging).is_ok());
            // Active -> Failed (valid)
            assert!(BranchStatus::Active.validate_transition(&BranchStatus::Failed).is_ok());
            // Completed -> Merging (valid)
            assert!(BranchStatus::Completed.validate_transition(&BranchStatus::Merging).is_ok());
            // Merging -> Merged (valid)
            assert!(BranchStatus::Merging.validate_transition(&BranchStatus::Merged).is_ok());
            // Merging -> Failed (valid)
            assert!(BranchStatus::Merging.validate_transition(&BranchStatus::Failed).is_ok());
        }
    }

    mod execution_strategy_tests {
        use super::*;

        #[test]
        fn sequential_validate_succeeds() {
            assert!(ExecutionStrategy::Sequential.validate().is_ok());
        }

        #[test]
        fn parallel_validate_succeeds_within_limit() {
            let strategy = ExecutionStrategy::Parallel { max_concurrent: 4 };
            assert!(strategy.validate().is_ok());
        }

        #[test]
        fn parallel_validate_succeeds_at_limit() {
            let strategy = ExecutionStrategy::Parallel {
                max_concurrent: MAX_CONCURRENT_BRANCHES,
            };
            assert!(strategy.validate().is_ok());
        }

        #[test]
        fn parallel_validate_fails_above_limit() {
            let strategy = ExecutionStrategy::Parallel {
                max_concurrent: MAX_CONCURRENT_BRANCHES + 1,
            };
            assert!(strategy.validate().is_err());
        }

        #[test]
        fn parallel_validate_fails_at_zero() {
            let strategy = ExecutionStrategy::Parallel { max_concurrent: 0 };
            assert!(strategy.validate().is_err());
        }

        #[test]
        fn sequential_concurrency_limit() {
            assert_eq!(ExecutionStrategy::Sequential.concurrency_limit(), 1);
        }

        #[test]
        fn parallel_concurrency_limit() {
            let strategy = ExecutionStrategy::Parallel { max_concurrent: 5 };
            assert_eq!(strategy.concurrency_limit(), 5);
        }
    }

    mod branch_config_tests {
        use super::*;

        #[test]
        fn config_new_succeeds_with_valid_data() {
            let config = BranchConfig::new(
                "test-branch",
                vec![],
                ExecutionStrategy::Sequential,
                false,
                "three-way",
            );
            assert!(config.is_ok());
        }

        #[test]
        fn config_new_fails_with_empty_name() {
            let config = BranchConfig::new(
                "",
                vec![],
                ExecutionStrategy::Sequential,
                false,
                "three-way",
            );
            assert!(matches!(
                config,
                Err(BranchValidationError::InvalidNameLength { .. })
            ));
        }

        #[test]
        fn config_new_fails_with_too_long_name() {
            let long_name = "a".repeat(MAX_BRANCH_NAME_LEN + 1);
            let config = BranchConfig::new(
                long_name,
                vec![],
                ExecutionStrategy::Sequential,
                false,
                "three-way",
            );
            assert!(matches!(
                config,
                Err(BranchValidationError::InvalidNameLength { .. })
            ));
        }

        #[test]
        fn config_new_fails_with_invalid_strategy() {
            let config = BranchConfig::new(
                "test",
                vec![],
                ExecutionStrategy::Parallel {
                    max_concurrent: 100,
                },
                false,
                "three-way",
            );
            assert!(matches!(
                config,
                Err(BranchValidationError::InvalidExecutionStrategy { .. })
            ));
        }
    }

    mod agent_assignment_tests {
        use super::*;

        #[test]
        fn assignment_new_succeeds() {
            let assignment = AgentAssignment::new("agent-1", None, Priority::DEFAULT);
            assert!(assignment.is_ok());
        }

        #[test]
        fn assignment_new_fails_with_empty_id() {
            let assignment = AgentAssignment::new("", None, Priority::DEFAULT);
            assert!(assignment.is_err());
        }

        #[test]
        fn assignment_preserves_values() {
            let assignment = AgentAssignment::new(
                "agent-1",
                Some("custom task".to_string()),
                Priority::new(200),
            )
            .unwrap();
            assert_eq!(assignment.agent_id().as_str(), "agent-1");
            assert_eq!(assignment.task_override(), Some("custom task"));
            assert_eq!(assignment.priority().inner(), 200);
        }
    }

    mod branch_tests {
        use super::*;

        fn create_test_config() -> BranchConfig {
            BranchConfig::new(
                "test-branch",
                vec![],
                ExecutionStrategy::Sequential,
                false,
                "three-way",
            )
            .unwrap()
        }

        fn create_test_branch() -> Branch {
            Branch::new(
                BranchId::new(),
                None,
                "session-123",
                "test-branch",
                create_test_config(),
            )
            .unwrap()
        }

        #[test]
        fn branch_new_succeeds() {
            let branch = create_test_branch();
            assert_eq!(branch.name(), "test-branch");
            assert_eq!(branch.session_id(), "session-123");
            assert!(branch.parent_id().is_none());
            assert!(matches!(branch.status(), BranchStatus::Pending));
        }

        #[test]
        fn branch_new_fails_with_empty_session() {
            let result = Branch::new(BranchId::new(), None, "", "test", create_test_config());
            assert!(matches!(result, Err(BranchValidationError::EmptySessionId)));
        }

        #[test]
        fn branch_new_fails_with_empty_name() {
            let result = Branch::new(BranchId::new(), None, "session", "", create_test_config());
            assert!(matches!(
                result,
                Err(BranchValidationError::InvalidNameLength { .. })
            ));
        }

        #[test]
        fn branch_with_parent() {
            let parent_id = BranchId::new();
            let branch = Branch::new(
                BranchId::new(),
                Some(parent_id),
                "session-123",
                "child-branch",
                create_test_config(),
            )
            .unwrap();
            assert_eq!(branch.parent_id(), Some(parent_id));
        }

        #[test]
        fn branch_add_child() {
            let mut parent = create_test_branch();
            let child_id = BranchId::new();
            parent.add_child(child_id);
            assert!(parent.children().contains(&child_id));
        }

        #[test]
        fn branch_add_child_deduplicates() {
            let mut parent = create_test_branch();
            let child_id = BranchId::new();
            parent.add_child(child_id);
            parent.add_child(child_id);
            assert_eq!(parent.children().len(), 1);
        }

        #[test]
        fn branch_remove_child() {
            let mut parent = create_test_branch();
            let child_id = BranchId::new();
            parent.add_child(child_id);
            parent.remove_child(child_id);
            assert!(!parent.children().contains(&child_id));
        }

        #[test]
        fn branch_start_execution_succeeds() {
            let mut branch = create_test_branch();
            assert!(branch.start_execution().is_ok());
            assert!(matches!(branch.status(), BranchStatus::Active));
        }

        #[test]
        fn branch_start_execution_fails_from_terminal() {
            let mut branch = create_test_branch();
            branch.fail("error").unwrap();
            assert!(branch.start_execution().is_err());
        }

        #[test]
        fn branch_complete_succeeds() {
            let mut branch = create_test_branch();
            branch.start_execution().unwrap();

            let result = BranchResult {
                branch_id: branch.id(),
                file_changes: vec![],
                agent_results: vec![],
                metrics: ExecutionMetrics {
                    total_duration_ms: 100,
                    files_processed: 5,
                    agents_executed: 2,
                    peak_memory_bytes: 1024,
                },
            };

            assert!(branch.complete(result).is_ok());
            assert!(branch.is_terminal());
            assert!(branch.completed_at().is_some());
            assert!(branch.execution_result().is_some());
        }

        #[test]
        fn branch_lifecycle_full_flow() {
            let mut branch = create_test_branch();

            // Start execution
            branch.start_execution().unwrap();
            assert!(branch.is_active());

            // Complete
            let result = BranchResult {
                branch_id: branch.id(),
                file_changes: vec![],
                agent_results: vec![],
                metrics: ExecutionMetrics {
                    total_duration_ms: 1000,
                    files_processed: 10,
                    agents_executed: 3,
                    peak_memory_bytes: 2048,
                },
            };
            branch.complete(result).unwrap();
            assert!(branch.is_terminal());
            assert!(!branch.is_active());
        }
    }

    mod merge_result_tests {
        use super::*;

        #[test]
        fn merge_result_has_conflicts_true() {
            let conflict = Conflict {
                file_path: PathBuf::from("test.txt"),
                conflict_type: ConflictType::Content,
                base_content: None,
                branch_contents: HashMap::new(),
            };
            let merge = MergeResult::new(vec![conflict], vec![], "strategy");
            assert!(merge.has_conflicts());
        }

        #[test]
        fn merge_result_has_conflicts_false() {
            let merge = MergeResult::new(vec![], vec![PathBuf::from("file.txt")], "strategy");
            assert!(!merge.has_conflicts());
        }
    }

    mod serialization_tests {
        use super::*;

        #[test]
        fn branch_id_serializes() {
            let id = BranchId::from_uuid(Uuid::nil());
            let json = serde_json::to_string(&id).unwrap();
            assert!(json.contains("00000000-0000-0000-0000-000000000000"));
        }

        #[test]
        fn branch_id_deserializes() {
            let json = "\"00000000-0000-0000-0000-000000000000\"";
            let id: BranchId = serde_json::from_str(json).unwrap();
            assert_eq!(id.inner(), Uuid::nil());
        }

        #[test]
        fn branch_status_serializes() {
            let status = BranchStatus::Active;
            let json = serde_json::to_string(&status).unwrap();
            assert!(json.contains("Active"));
        }

        #[test]
        fn execution_strategy_serializes() {
            let strategy = ExecutionStrategy::Parallel { max_concurrent: 4 };
            let json = serde_json::to_string(&strategy).unwrap();
            assert!(json.contains("Parallel"));
            assert!(json.contains("4"));
        }

        #[test]
        fn branch_serializes() {
            let branch = Branch::new(
                BranchId::from_uuid(Uuid::nil()),
                None,
                "session-1",
                "test-branch",
                BranchConfig::default(),
            )
            .unwrap();

            let json = serde_json::to_string_pretty(&branch).unwrap();
            assert!(json.contains("test-branch"));
            assert!(json.contains("session-1"));
            assert!(json.contains("Pending"));
        }

        #[test]
        fn branch_roundtrip() {
            let branch = Branch::new(
                BranchId::from_uuid(Uuid::nil()),
                Some(BranchId::from_uuid(Uuid::nil())),
                "session-1",
                "test-branch",
                BranchConfig::new(
                    "config-name",
                    vec![AgentAssignment::new("agent-1", None, Priority::new(100)).unwrap()],
                    ExecutionStrategy::Parallel { max_concurrent: 4 },
                    true,
                    "fast-forward",
                )
                .unwrap(),
            )
            .unwrap();

            let json = serde_json::to_string(&branch).unwrap();
            let deserialized: Branch = serde_json::from_str(&json).unwrap();

            assert_eq!(branch.id(), deserialized.id());
            assert_eq!(branch.name(), deserialized.name());
            assert_eq!(branch.session_id(), deserialized.session_id());
            assert_eq!(branch.parent_id(), deserialized.parent_id());
        }

        #[test]
        fn conflict_roundtrip() {
            let mut branch_contents = HashMap::new();
            branch_contents.insert(BranchId::new(), "content1".to_string());
            branch_contents.insert(BranchId::new(), "content2".to_string());

            let conflict = Conflict {
                file_path: PathBuf::from("src/main.rs"),
                conflict_type: ConflictType::Content,
                base_content: Some("base".to_string()),
                branch_contents,
            };

            let json = serde_json::to_string(&conflict).unwrap();
            let deserialized: Conflict = serde_json::from_str(&json).unwrap();

            assert_eq!(conflict.file_path, deserialized.file_path);
            assert_eq!(conflict.conflict_type, deserialized.conflict_type);
            assert_eq!(
                conflict.branch_contents.len(),
                deserialized.branch_contents.len()
            );
        }
    }

    mod session_snapshot_tests {
        use super::*;

        #[test]
        fn snapshot_new_succeeds() {
            let snapshot = SessionSnapshot::new("session-1", Utc::now(), Some("test".to_string()));
            assert!(snapshot.is_ok());
        }

        #[test]
        fn snapshot_new_fails_with_empty_session() {
            let snapshot = SessionSnapshot::new("", Utc::now(), None);
            assert!(matches!(
                snapshot,
                Err(BranchValidationError::EmptySessionId)
            ));
        }

        #[test]
        fn snapshot_roundtrip() {
            let now = Utc::now();
            let snapshot =
                SessionSnapshot::new("session-1", now, Some("description".to_string())).unwrap();

            let json = serde_json::to_string(&snapshot).unwrap();
            let deserialized: SessionSnapshot = serde_json::from_str(&json).unwrap();

            assert_eq!(snapshot.session_id, deserialized.session_id);
            assert_eq!(snapshot.description, deserialized.description);
        }
    }
}
