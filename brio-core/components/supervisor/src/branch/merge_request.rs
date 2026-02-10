//! Merge Request - Merge request creation, approval, and execution.
//!
//! This module handles the git-like merge workflow:
//! - Merge request creation
//! - Merge approval
//! - Merge execution
//! - Merge commit

use tracing::{info, instrument, warn};

use crate::branch::{Branch, BranchError, BranchManager, MergeRequestId, SessionError};
use crate::domain::{
    BranchId, BranchStatus, ChangeType, Conflict, MergeRequestStatus, StagedChange,
};
use crate::merge::{FileChange as MergeFileChange, MergeResult as MergeOutput};
use crate::repository::BranchRepositoryError;

impl BranchManager {
    /// Requests a merge for a branch.
    ///
    /// If auto-merge is enabled and no approval is required, the merge is executed immediately.
    /// Otherwise, a merge request is created for later approval.
    ///
    /// # Errors
    ///
    /// Returns `BranchError` if:
    /// - Branch not found
    /// - Branch is not in Completed state
    /// - Invalid merge strategy
    /// - Repository operation fails
    #[instrument(skip(self, branch_id, strategy))]
    pub async fn request_merge(
        &self,
        branch_id: BranchId,
        strategy: &str,
        requires_approval: bool,
    ) -> Result<MergeRequestId, BranchError> {
        // 1. Get branch
        let branch = self
            .repository
            .get_branch(branch_id)?
            .ok_or(BranchError::BranchNotFound(branch_id))?;

        // 2. Validate branch is Completed
        if branch.status() != BranchStatus::Completed {
            return Err(BranchError::InvalidBranchState {
                branch_id,
                expected: "Completed".to_string(),
                actual: format!("{:?}", branch.status()),
            });
        }

        // 3. Validate strategy exists
        if self.merge_registry.get(strategy).is_none() {
            return Err(BranchError::InvalidStrategy(strategy.to_string()));
        }

        // 4. Create merge request
        let parent_id = branch.parent_id();
        let merge_id = self
            .repository
            .create_merge_request(branch_id, parent_id, strategy)?;

        // 5. Auto-approve if approval is not required
        if !requires_approval {
            let auto_approver = "auto";
            self.repository.approve_merge(merge_id, auto_approver)?;
            info!(
                "Auto-approved merge request {} for branch {} (approval not required)",
                merge_id, branch_id
            );
        }

        info!(
            "Created merge request {} for branch {} with strategy '{}'",
            merge_id, branch_id, strategy
        );

        Ok(merge_id)
    }

    /// Executes a merge after approval using a git-like workflow.
    ///
    /// This implements a staged merge process:
    /// 1. Find merge request and validate it's approved
    /// 2. Get branch and validate it's Completed
    /// 3. Find parent branch (or use base path)
    /// 4. Create staging session for merge
    /// 5. Collect file changes from branch session
    /// 6. Detect conflicts using merge strategies
    /// 7. Apply non-conflicting changes to staging
    /// 8. If conflicts exist, mark as `HasConflicts` and return
    /// 9. If no conflicts, mark as `ReadyToCommit`
    ///
    /// # Errors
    ///
    /// Returns `BranchError` if:
    /// - Merge request not found
    /// - Merge request not approved
    /// - Branch not found or not in Completed state
    /// - Strategy not found
    /// - Merge execution fails
    /// - Session creation fails
    #[instrument(skip(self, merge_request_id))]
    pub async fn execute_merge(
        &self,
        merge_request_id: MergeRequestId,
    ) -> Result<MergeOutput, BranchError> {
        use crate::domain::BranchValidationError;

        // 1. Get merge request
        let merge_request = self
            .repository
            .get_merge_request(merge_request_id)
            .map_err(BranchError::Repository)?
            .ok_or(BranchError::MergeRequestNotFound(merge_request_id))?;

        // 2. Validate merge request is approved
        if !merge_request.is_approved() {
            return Err(BranchError::MergeNotApproved(merge_request_id));
        }

        // 3. Get branch being merged
        let branch_id = merge_request.branch_id();
        let branch_record = self
            .repository
            .get_branch(branch_id)?
            .ok_or(BranchError::BranchNotFound(branch_id))?;

        // 4. Validate branch is in Completed state
        if branch_record.status() != BranchStatus::Completed {
            return Err(BranchError::InvalidBranchState {
                branch_id,
                expected: "Completed".to_string(),
                actual: format!("{:?}", branch_record.status()),
            });
        }

        // Convert to domain entity
        let branch = Branch::try_from_record(&branch_record).map_err(BranchError::Validation)?;

        // 5. Get parent path for merge destination
        let parent_path = if let Some(parent_id) = branch_record.parent_id() {
            let parent = self
                .repository
                .get_branch(parent_id)?
                .ok_or(BranchError::BranchNotFound(parent_id))?;
            let session_manager = self.lock_session_manager()?;
            session_manager
                .session_path(parent.session_id())
                .ok_or_else(|| {
                    BranchError::Session(SessionError::SessionNotFound(
                        parent.session_id().to_string(),
                    ))
                })?
        } else {
            // TODO(#123): Implement root branch merge with base path support
            return Err(BranchError::Validation(
                BranchValidationError::InvalidExecutionStrategy {
                    reason: "Root branch merge not implemented".to_string(),
                },
            ));
        };

        // 6. Create staging session for merge
        let parent_path_str = parent_path.to_string_lossy().to_string();
        let staging_session_id = {
            let mut session_manager = self.lock_session_manager()?;
            session_manager.begin_session(&parent_path_str)?
        };

        // Execute merge operations with cleanup on error
        let merge_result = async {
            // 7. Collect file changes from branch session
            let branch_changes = self.collect_branch_changes(&branch).await?;

            // 8. Get the strategy and perform merge
            let strategy_name = merge_request.strategy();
            let strategy = self
                .merge_registry
                .get(strategy_name)
                .ok_or_else(|| BranchError::InvalidStrategy(strategy_name.to_string()))?;

            // Create branch result for merge strategy
            let branch_result =
                crate::merge::BranchResult::new(branch_id, parent_path.clone(), branch_changes);

            // Execute merge strategy
            let merge_result = strategy
                .merge(&parent_path, &[branch_result])
                .await
                .map_err(BranchError::Merge)?;

            Ok::<_, BranchError>(merge_result)
        }
        .await;

        // Clean up staging session if an error occurred
        if merge_result.is_err() {
            if let Ok(mut session_manager) = self.lock_session_manager() {
                let _ = session_manager.rollback_session(&staging_session_id);
            }
            return merge_result;
        }

        let merge_result = merge_result.unwrap();

        // 9. Convert merge result to staged changes
        let staged_changes: Vec<StagedChange> = merge_result
            .merged_changes
            .iter()
            .map(|change| StagedChange {
                path: change.path().to_path_buf(),
                change_type: match change {
                    MergeFileChange::Added(_) => ChangeType::Added,
                    MergeFileChange::Modified(_) => ChangeType::Modified,
                    MergeFileChange::Deleted(_) => ChangeType::Deleted,
                },
                content_hash: None, // Could compute hash here if needed
            })
            .collect();

        // 10. Apply non-conflicting changes to staging area
        if !merge_result.has_conflicts() {
            // Apply all changes to staging session
            if let Err(e) = self
                .apply_changes_to_staging(&staging_session_id, &merge_result.merged_changes)
                .await
            {
                // Clean up staging session on error
                if let Ok(mut session_manager) = self.lock_session_manager() {
                    let _ = session_manager.rollback_session(&staging_session_id);
                }
                return Err(e);
            }
        }

        // 11. Update merge request status
        let conflicts: Vec<Conflict> = merge_result
            .conflicts
            .iter()
            .map(|c| {
                Conflict::new(
                    c.path().clone(),
                    crate::domain::ConflictType::Content,
                    None,
                    std::collections::HashMap::new(),
                )
            })
            .collect();

        // Build updated merge request
        let mut updated_merge_request = merge_request.clone();
        updated_merge_request.start(staging_session_id.clone(), chrono::Utc::now().timestamp());
        updated_merge_request.set_staged_changes(staged_changes);
        updated_merge_request.set_conflicts(conflicts);

        // Save updated merge request
        if let Err(e) = self
            .repository
            .update_merge_request(&updated_merge_request)
            .map_err(BranchError::Repository)
        {
            // Clean up staging session on error
            if let Ok(mut session_manager) = self.lock_session_manager() {
                let _ = session_manager.rollback_session(&staging_session_id);
            }
            return Err(e);
        }

        // 12. Update branch status to Merging
        if let Err(e) = self.update_status(branch_id, BranchStatus::Merging) {
            // Clean up staging session on error
            if let Ok(mut session_manager) = self.lock_session_manager() {
                let _ = session_manager.rollback_session(&staging_session_id);
            }
            return Err(e);
        }

        info!(
            "Executed merge {} for branch {}: {} changes, {} conflicts",
            merge_request_id,
            branch_id,
            merge_result.merged_changes.len(),
            merge_result.conflicts.len()
        );

        // 13. Return merge output
        Ok(merge_result)
    }

    /// Commits a staged merge to the parent branch.
    ///
    /// This is the final step in the git-like merge workflow, similar to `git commit`.
    /// It applies all staged changes to the parent branch and marks the merge as complete.
    ///
    /// # Errors
    ///
    /// Returns `BranchError` if:
    /// - Merge request not found
    /// - Merge not in `ReadyToCommit` state
    /// - Session commit fails
    /// - Repository update fails
    #[instrument(skip(self, merge_request_id))]
    pub async fn commit_merge(&self, merge_request_id: MergeRequestId) -> Result<(), BranchError> {
        // 1. Get merge request
        let merge_request = self
            .repository
            .get_merge_request(merge_request_id)
            .map_err(BranchError::Repository)?
            .ok_or(BranchError::MergeRequestNotFound(merge_request_id))?;

        // 2. Validate merge is ready to commit
        if merge_request.status() != MergeRequestStatus::ReadyToCommit {
            return Err(BranchError::InvalidBranchState {
                branch_id: merge_request.branch_id(),
                expected: "ReadyToCommit".to_string(),
                actual: format!("{:?}", merge_request.status()),
            });
        }

        // 3. Get staging session ID
        let staging_session_id = merge_request.staging_session_id().ok_or_else(|| {
            BranchError::Session(SessionError::SessionNotFound(
                "Merge staging session not found".to_string(),
            ))
        })?;

        // 4. Commit staging session to parent
        {
            let mut session_manager = self.lock_session_manager()?;
            session_manager.commit_session(staging_session_id)?;
        }

        // 5. Get branch and mark as Merged
        let branch_id = merge_request.branch_id();
        self.update_status(branch_id, BranchStatus::Merged)?;

        // 6. Mark merge request as committed
        let mut updated_merge_request = merge_request.clone();
        updated_merge_request.mark_committed(chrono::Utc::now().timestamp());
        self.repository
            .update_merge_request(&updated_merge_request)
            .map_err(BranchError::Repository)?;

        info!(
            "Committed merge {} for branch {}",
            merge_request_id, branch_id
        );

        Ok(())
    }

    /// Approves a pending merge request.
    ///
    /// # Errors
    ///
    /// Returns `BranchError` if:
    /// - Merge request not found
    /// - Repository update fails
    #[instrument(skip(self, merge_request_id))]
    pub fn approve_merge(
        &self,
        merge_request_id: MergeRequestId,
        approver: &str,
    ) -> Result<(), BranchError> {
        self.repository
            .approve_merge(merge_request_id, approver)
            .map_err(|e| match e {
                BranchRepositoryError::BranchNotFound(_) => {
                    BranchError::MergeRequestNotFound(merge_request_id)
                }
                _ => BranchError::Repository(e),
            })?;

        info!(
            "Approved merge request {} by {}",
            merge_request_id, approver
        );

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{BranchConfig, ExecutionStrategy};

    fn create_test_config() -> BranchConfig {
        BranchConfig::new(
            "test-branch",
            vec![],
            ExecutionStrategy::Sequential,
            false,
            "union",
        )
        .unwrap()
    }
}
