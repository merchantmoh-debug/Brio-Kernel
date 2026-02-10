//! Branch Domain Types
//!
//! Core domain types for the branching orchestrator system.
//! All types follow the principle of making invalid states unrepresentable.

pub mod coordinator;
pub mod lifecycle;
pub mod merge_request;
pub mod operations;
pub mod state_machine;

use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// Re-export primary types from coordinator module
pub use coordinator::{
    BranchError, BranchManager, BranchTree, MergeRequestId, SessionError, SessionManager,
};

// Re-export domain types that are shared
pub use crate::domain::{
    AgentAssignment, AgentResult, BranchConfig, BranchId, BranchRecord, BranchResult, BranchStatus,
    BranchValidationError, ChangeType, Conflict, ConflictType, ExecutionMetrics, ExecutionStrategy,
    FileChange, MAX_BRANCH_NAME_LEN, MAX_CONCURRENT_BRANCHES, MIN_BRANCH_NAME_LEN, MergeResult,
    Priority, StagedChange,
};

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
    /// Returns `BranchValidationError::EmptySessionId` if `session_id` is empty.
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
        if !(MIN_BRANCH_NAME_LEN..=MAX_BRANCH_NAME_LEN).contains(&name_len) {
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
    pub fn status(&self) -> BranchStatus {
        self.status
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
        use crate::domain::BranchValidationError;

        // Use the validate_transition method from BranchStatus
        if let Err(_e) = self.status.validate_transition(&new_status) {
            return Err(BranchValidationError::InvalidStatusTransition {
                from: self.status,
                to: new_status,
            });
        }

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

    /// Creates a Branch from a `BranchRecord` (database representation).
    ///
    /// # Errors
    /// Returns `BranchValidationError` if the record is invalid or config cannot be parsed.
    pub fn try_from_record(record: &BranchRecord) -> Result<Self, BranchValidationError> {
        use crate::domain::BranchConfig;

        let config: BranchConfig = serde_json::from_str(record.config()).map_err(|e| {
            BranchValidationError::InvalidExecutionStrategy {
                reason: format!("Failed to parse config: {e}"),
            }
        })?;

        let created_at =
            chrono::DateTime::from_timestamp(record.created_at(), 0).ok_or_else(|| {
                BranchValidationError::InvalidTimestamp {
                    field: "created_at".to_string(),
                    value: record.created_at(),
                }
            })?;

        let completed_at = match record.completed_at() {
            Some(ts) => Some(chrono::DateTime::from_timestamp(ts, 0).ok_or_else(|| {
                BranchValidationError::InvalidTimestamp {
                    field: "completed_at".to_string(),
                    value: ts,
                }
            })?),
            None => None,
        };

        Ok(Self {
            id: record.id(),
            parent_id: record.parent_id(),
            session_id: record.session_id().to_string(),
            name: record.name().to_string(),
            status: record.status(),
            children: Vec::new(),
            config,
            created_at,
            completed_at,
            execution_result: None,
            merge_result: None,
            failure_reason: None,
        })
    }
}

#[cfg(test)]
mod tests {
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

        let result = BranchResult::new(
            branch.id(),
            vec![],
            vec![],
            ExecutionMetrics {
                total_duration_ms: 100,
                files_processed: 5,
                agents_executed: 2,
                peak_memory_bytes: 1024,
            },
        );

        assert!(branch.complete(result).is_ok());
        assert!(branch.is_terminal());
        assert!(branch.completed_at().is_some());
        assert!(branch.execution_result().is_some());
    }

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
