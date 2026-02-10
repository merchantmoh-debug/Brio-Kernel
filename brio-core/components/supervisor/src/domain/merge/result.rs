//! Merge result types.
//!
//! This module defines types for merge results and merge requests.

use crate::domain::ids::BranchId;
use crate::merge::MergeId;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use super::change::StagedChange;
use super::conflict::Conflict;

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

    /// Returns the number of conflicts.
    #[must_use]
    pub fn conflict_count(&self) -> usize {
        self.conflicts.len()
    }

    /// Returns the number of resolved files.
    #[must_use]
    pub fn resolved_count(&self) -> usize {
        self.resolved_files.len()
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

    /// Checks if this status represents a terminal state.
    #[must_use]
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Committed | Self::Rejected)
    }

    /// Checks if this status represents an active merge.
    #[must_use]
    pub fn is_active(&self) -> bool {
        matches!(
            self,
            Self::InProgress | Self::HasConflicts | Self::ReadyToCommit
        )
    }
}

/// Enhanced merge request entity with git-like staging workflow.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MergeRequest {
    id: MergeId,
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
    /// Constructs a new `MergeRequest` (factory method).
    #[must_use]
    pub fn new(
        id: MergeId,
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
    pub const fn id(&self) -> MergeId {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::ids::BranchId;
    use crate::merge::MergeId;
    use std::collections::HashMap;

    #[test]
    fn test_merge_result_creation() {
        let result = MergeResult::new(vec![], vec![PathBuf::from("file.rs")], "three-way");

        assert!(!result.has_conflicts());
        assert_eq!(result.conflict_count(), 0);
        assert_eq!(result.resolved_count(), 1);
        assert_eq!(result.strategy_used, "three-way");
    }

    #[test]
    fn test_merge_result_with_conflicts() {
        let conflict = Conflict::new(
            PathBuf::from("conflict.rs"),
            super::super::conflict::ConflictType::Content,
            None,
            HashMap::new(),
        );
        let result = MergeResult::new(vec![conflict], vec![], "strategy");

        assert!(result.has_conflicts());
        assert_eq!(result.conflict_count(), 1);
        assert_eq!(result.resolved_count(), 0);
    }

    #[test]
    fn test_merge_request_status_transitions() {
        assert!(MergeRequestStatus::Pending.can_transition_to(MergeRequestStatus::Approved));
        assert!(MergeRequestStatus::Pending.can_transition_to(MergeRequestStatus::Rejected));
        assert!(!MergeRequestStatus::Rejected.can_transition_to(MergeRequestStatus::Approved));

        assert!(MergeRequestStatus::Approved.can_transition_to(MergeRequestStatus::InProgress));
        assert!(MergeRequestStatus::InProgress.can_transition_to(MergeRequestStatus::HasConflicts));
        assert!(
            MergeRequestStatus::HasConflicts.can_transition_to(MergeRequestStatus::ReadyToCommit)
        );
        assert!(MergeRequestStatus::ReadyToCommit.can_transition_to(MergeRequestStatus::Committed));
    }

    #[test]
    fn test_merge_request_status_terminal() {
        assert!(MergeRequestStatus::Committed.is_terminal());
        assert!(MergeRequestStatus::Rejected.is_terminal());
        assert!(!MergeRequestStatus::Pending.is_terminal());
        assert!(!MergeRequestStatus::InProgress.is_terminal());
    }

    #[test]
    fn test_merge_request_status_active() {
        assert!(MergeRequestStatus::InProgress.is_active());
        assert!(MergeRequestStatus::HasConflicts.is_active());
        assert!(MergeRequestStatus::ReadyToCommit.is_active());
        assert!(!MergeRequestStatus::Pending.is_active());
        assert!(!MergeRequestStatus::Committed.is_active());
    }

    #[test]
    fn test_merge_request_lifecycle() {
        let mut request = MergeRequest::new(
            MergeId::new(),
            BranchId::new(),
            None,
            "three-way",
            true,
            1000,
        );

        assert!(matches!(request.status(), MergeRequestStatus::Pending));
        assert!(request.requires_approval());

        request.approve("user1", 2000);
        assert!(matches!(request.status(), MergeRequestStatus::Approved));
        assert_eq!(request.approved_by(), Some("user1"));
        assert_eq!(request.approved_at(), Some(2000));

        request.start("session-1", 3000);
        assert!(matches!(request.status(), MergeRequestStatus::InProgress));
        assert_eq!(request.staging_session_id(), Some("session-1"));
        assert_eq!(request.started_at(), Some(3000));

        request.mark_committed(4000);
        assert!(matches!(request.status(), MergeRequestStatus::Committed));
        assert_eq!(request.completed_at(), Some(4000));
    }

    #[test]
    fn test_merge_request_with_conflicts() {
        let mut request = MergeRequest::new(
            MergeId::new(),
            BranchId::new(),
            None,
            "three-way",
            false,
            1000,
        );

        request.start("session-1", 2000);

        let conflict = Conflict::new(
            PathBuf::from("file.rs"),
            super::super::conflict::ConflictType::Content,
            None,
            HashMap::new(),
        );

        request.set_conflicts(vec![conflict]);
        assert!(matches!(request.status(), MergeRequestStatus::HasConflicts));
        assert!(request.has_conflicts());

        request.mark_conflicts_resolved();
        assert!(matches!(
            request.status(),
            MergeRequestStatus::ReadyToCommit
        ));
    }

    #[test]
    fn test_merge_status_variants() {
        assert!(matches!(MergeStatus::Pending, MergeStatus::Pending));
        assert!(matches!(MergeStatus::Approved, MergeStatus::Approved));
        assert!(matches!(MergeStatus::Rejected, MergeStatus::Rejected));
        assert!(matches!(MergeStatus::Merged, MergeStatus::Merged));
        assert!(matches!(MergeStatus::Conflict, MergeStatus::Conflict));
    }
}
