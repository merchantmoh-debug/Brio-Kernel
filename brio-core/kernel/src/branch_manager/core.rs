//! Core branch manager for the Brio kernel.
//!
//! This module provides the `BranchManager` which orchestrates branch operations.

use chrono::Utc;

use super::storage::BranchStorage;
use super::types::{
    AgentAssignment, Branch, BranchConfig, BranchError, BranchId, BranchStatus, ExecutionStrategy,
    MergeRequestId, MergeRequestModel, MergeRequestStatus,
};

/// Manager for branch operations.
#[derive(Debug, Default)]
pub struct BranchManager {
    /// In-memory storage for branches and merge requests.
    storage: BranchStorage,
}

impl BranchManager {
    /// Create a new branch manager.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a new branch with explicit storage (dependency injection).
    #[must_use]
    pub fn with_storage(storage: BranchStorage) -> Self {
        Self { storage }
    }

    /// Create a new branch.
    ///
    /// # Errors
    ///
    /// Returns an error if the branch cannot be created.
    pub async fn create_branch(
        &self,
        name: String,
        agents: Vec<AgentAssignment>,
        execution_strategy: ExecutionStrategy,
        auto_merge: bool,
        merge_strategy: String,
    ) -> Result<Branch, BranchError> {
        let id = BranchId::new(uuid::Uuid::new_v4().to_string())
            .map_err(|e| BranchError::Internal(e.to_string()))?;

        // Check for duplicate name
        if self.storage.branch_name_exists(&name) {
            return Err(BranchError::BranchAlreadyExists(name.clone()));
        }

        let branch = Branch {
            id,
            parent_id: None, // TODO: Set based on source
            name,
            status: BranchStatus::Pending,
            session_id: uuid::Uuid::new_v4().to_string(),
            created_at: Utc::now(),
            completed_at: None,
            children: Vec::new(),
            config: BranchConfig {
                agents,
                execution_strategy,
                auto_merge,
                merge_strategy,
            },
        };

        self.storage.insert_branch(branch.clone());

        Ok(branch)
    }

    /// Get a branch by ID.
    ///
    /// # Errors
    ///
    /// Returns an error if the branch is not found.
    pub async fn get_branch(&self, id: &BranchId) -> Result<Branch, BranchError> {
        self.storage
            .get_branch(id)
            .ok_or_else(|| BranchError::BranchNotFound(id.to_string()))
    }

    /// List branches with optional filters.
    ///
    /// # Errors
    ///
    /// Returns an error if the database query fails.
    pub async fn list_branches(
        &self,
        status_filter: Option<&str>,
        parent_id_filter: Option<&BranchId>,
    ) -> Result<Vec<Branch>, BranchError> {
        let mut result: Vec<Branch> = self
            .storage
            .get_all_branches()
            .into_iter()
            .filter(|b| {
                if let Some(status) = status_filter {
                    b.status.to_string() == status
                } else {
                    true
                }
            })
            .filter(|b| {
                if let Some(parent_id) = parent_id_filter {
                    b.parent_id
                        .as_ref()
                        .is_some_and(|p| p.as_str() == parent_id.as_str())
                } else {
                    true
                }
            })
            .collect();

        // Sort by created_at descending
        result.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        Ok(result)
    }

    /// Delete a branch.
    ///
    /// # Errors
    ///
    /// Returns an error if the branch is not found or cannot be deleted.
    pub async fn delete_branch(&self, id: &BranchId) -> Result<(), BranchError> {
        if !self.storage.contains_branch(id) {
            return Err(BranchError::BranchNotFound(id.to_string()));
        }
        self.storage.remove_branch(id);
        Ok(())
    }

    /// Execute a branch.
    ///
    /// # Errors
    ///
    /// Returns an error if the branch is not found or execution fails.
    pub async fn execute_branch(
        &self,
        id: &BranchId,
        _agent_filter: Option<Vec<String>>,
        _task_override: Option<String>,
    ) -> Result<Branch, BranchError> {
        let mut branch = self
            .storage
            .get_branch_mut(id)
            .ok_or_else(|| BranchError::BranchNotFound(id.to_string()))?;

        if branch.status != BranchStatus::Pending && branch.status != BranchStatus::Failed {
            return Err(BranchError::InvalidStateTransition {
                from: branch.status.to_string(),
                to: "running".to_string(),
            });
        }

        branch.status = BranchStatus::Running;
        // TODO: Actually execute the branch
        branch.status = BranchStatus::Completed;
        branch.completed_at = Some(Utc::now());

        Ok(branch.clone())
    }

    /// Abort a branch.
    ///
    /// # Errors
    ///
    /// Returns an error if the branch is not found or cannot be aborted.
    pub async fn abort_branch(&self, id: &BranchId) -> Result<Branch, BranchError> {
        let mut branch = self
            .storage
            .get_branch_mut(id)
            .ok_or_else(|| BranchError::BranchNotFound(id.to_string()))?;

        if branch.status != BranchStatus::Running && branch.status != BranchStatus::Pending {
            return Err(BranchError::InvalidStateTransition {
                from: branch.status.to_string(),
                to: "aborted".to_string(),
            });
        }

        branch.status = BranchStatus::Aborted;
        branch.completed_at = Some(Utc::now());

        Ok(branch.clone())
    }

    /// Request a merge for a branch.
    ///
    /// # Errors
    ///
    /// Returns an error if the branch is not found or cannot be merged.
    pub async fn request_merge(
        &self,
        branch_id: &BranchId,
        strategy: String,
        requires_approval: bool,
    ) -> Result<MergeRequestModel, BranchError> {
        if self.storage.get_branch(branch_id).is_none() {
            return Err(BranchError::BranchNotFound(branch_id.to_string()));
        }

        let merge_request_id = MergeRequestId::new(uuid::Uuid::new_v4().to_string())
            .map_err(|e| BranchError::Internal(e.to_string()))?;

        let merge_request = MergeRequestModel {
            id: merge_request_id,
            branch_id: branch_id.clone(),
            strategy,
            status: MergeRequestStatus::Pending,
            requires_approval,
            approved_by: None,
            approved_at: None,
        };

        self.storage.insert_merge_request(merge_request.clone());

        Ok(merge_request)
    }

    /// Approve a merge request.
    ///
    /// # Errors
    ///
    /// Returns an error if the merge request is not found.
    pub async fn approve_merge(
        &self,
        merge_request_id: &MergeRequestId,
        approver: String,
    ) -> Result<MergeRequestModel, BranchError> {
        let mut merge_request = self
            .storage
            .get_merge_request_mut(merge_request_id)
            .ok_or_else(|| BranchError::BranchNotFound(merge_request_id.to_string()))?;

        merge_request.status = MergeRequestStatus::Approved;
        merge_request.approved_by = Some(approver);
        merge_request.approved_at = Some(Utc::now());

        Ok(merge_request.clone())
    }

    /// Reject a merge request.
    ///
    /// # Errors
    ///
    /// Returns an error if the merge request is not found.
    pub async fn reject_merge(
        &self,
        merge_request_id: &MergeRequestId,
    ) -> Result<MergeRequestModel, BranchError> {
        let mut merge_request = self
            .storage
            .get_merge_request_mut(merge_request_id)
            .ok_or_else(|| BranchError::BranchNotFound(merge_request_id.to_string()))?;

        merge_request.status = MergeRequestStatus::Rejected;

        Ok(merge_request.clone())
    }

    /// Get the merge request for a branch.
    ///
    /// # Errors
    ///
    /// Returns an error if the merge request is not found.
    pub async fn get_merge_request(
        &self,
        merge_request_id: &MergeRequestId,
    ) -> Result<MergeRequestModel, BranchError> {
        self.storage
            .get_merge_request(merge_request_id)
            .ok_or_else(|| BranchError::BranchNotFound(merge_request_id.to_string()))
    }

    /// Get all merge requests for a branch.
    ///
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub async fn get_branch_merge_requests(
        &self,
        branch_id: &BranchId,
    ) -> Result<Vec<MergeRequestModel>, BranchError> {
        Ok(self.storage.get_merge_requests_for_branch(branch_id))
    }

    /// Get the branch tree.
    ///
    /// # Errors
    ///
    /// Returns an error if the branch is not found.
    pub async fn get_branch_tree(&self, id: &BranchId) -> Result<Branch, BranchError> {
        self.get_branch(id).await
    }
}
