//! Branch storage for the Brio kernel.
//!
//! This module provides in-memory storage for branches and merge requests.

use parking_lot::RwLock;
use std::collections::HashMap;

use super::types::{Branch, BranchId, MergeRequestId, MergeRequestModel};

/// Storage for branches and merge requests.
#[derive(Debug, Default)]
pub struct BranchStorage {
    /// In-memory storage for branches (temporary - will use database).
    branches: RwLock<HashMap<String, Branch>>,
    /// In-memory storage for merge requests (temporary - will use database).
    merge_requests: RwLock<HashMap<String, MergeRequestModel>>,
}

impl BranchStorage {
    /// Create new storage instance.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert a branch into storage.
    pub fn insert_branch(&self, branch: Branch) {
        self.branches
            .write()
            .insert(branch.id.as_str().to_string(), branch);
    }

    /// Get a branch by ID.
    pub fn get_branch(&self, id: &BranchId) -> Option<Branch> {
        self.branches.read().get(id.as_str()).cloned()
    }

    /// Get a mutable reference to a branch.
    pub fn get_branch_mut(
        &self,
        id: &BranchId,
    ) -> Option<parking_lot::MappedRwLockWriteGuard<'_, Branch>> {
        use parking_lot::RwLockWriteGuard;
        let branches = self.branches.write();
        RwLockWriteGuard::try_map(branches, |branches| branches.get_mut(id.as_str())).ok()
    }

    /// Check if a branch exists.
    pub fn contains_branch(&self, id: &BranchId) -> bool {
        self.branches.read().contains_key(id.as_str())
    }

    /// Remove a branch from storage.
    pub fn remove_branch(&self, id: &BranchId) {
        self.branches.write().remove(id.as_str());
    }

    /// Get all branches.
    pub fn get_all_branches(&self) -> Vec<Branch> {
        self.branches.read().values().cloned().collect()
    }

    /// Insert a merge request into storage.
    pub fn insert_merge_request(&self, merge_request: MergeRequestModel) {
        self.merge_requests
            .write()
            .insert(merge_request.id.as_str().to_string(), merge_request);
    }

    /// Get a merge request by ID.
    pub fn get_merge_request(&self, id: &MergeRequestId) -> Option<MergeRequestModel> {
        self.merge_requests.read().get(id.as_str()).cloned()
    }

    /// Get a mutable reference to a merge request.
    pub fn get_merge_request_mut(
        &self,
        id: &MergeRequestId,
    ) -> Option<parking_lot::MappedRwLockWriteGuard<'_, MergeRequestModel>> {
        use parking_lot::RwLockWriteGuard;
        let merge_requests = self.merge_requests.write();
        RwLockWriteGuard::try_map(merge_requests, |mrs| mrs.get_mut(id.as_str())).ok()
    }

    /// Get all merge requests for a specific branch.
    pub fn get_merge_requests_for_branch(&self, branch_id: &BranchId) -> Vec<MergeRequestModel> {
        self.merge_requests
            .read()
            .values()
            .filter(|mr| mr.branch_id.as_str() == branch_id.as_str())
            .cloned()
            .collect()
    }

    /// Check if a branch name already exists.
    pub fn branch_name_exists(&self, name: &str) -> bool {
        self.branches.read().values().any(|b| b.name == name)
    }
}

/// Trait for branch storage operations.
pub trait BranchStoragePort: Send + Sync {
    /// Insert a branch.
    fn insert_branch(&self, branch: Branch);
    /// Get a branch by ID.
    fn get_branch(&self, id: &BranchId) -> Option<Branch>;
    /// Get all branches.
    fn get_all_branches(&self) -> Vec<Branch>;
    /// Remove a branch.
    fn remove_branch(&self, id: &BranchId);
    /// Check if branch name exists.
    fn branch_name_exists(&self, name: &str) -> bool;
}

/// Trait for merge request storage operations.
pub trait MergeRequestStoragePort: Send + Sync {
    /// Insert a merge request.
    fn insert_merge_request(&self, merge_request: MergeRequestModel);
    /// Get a merge request by ID.
    fn get_merge_request(&self, id: &MergeRequestId) -> Option<MergeRequestModel>;
    /// Get all merge requests for a branch.
    fn get_merge_requests_for_branch(&self, branch_id: &BranchId) -> Vec<MergeRequestModel>;
}

impl BranchStoragePort for BranchStorage {
    fn insert_branch(&self, branch: Branch) {
        self.insert_branch(branch);
    }

    fn get_branch(&self, id: &BranchId) -> Option<Branch> {
        self.get_branch(id)
    }

    fn get_all_branches(&self) -> Vec<Branch> {
        self.get_all_branches()
    }

    fn remove_branch(&self, id: &BranchId) {
        self.remove_branch(id);
    }

    fn branch_name_exists(&self, name: &str) -> bool {
        self.branch_name_exists(name)
    }
}

impl MergeRequestStoragePort for BranchStorage {
    fn insert_merge_request(&self, merge_request: MergeRequestModel) {
        self.insert_merge_request(merge_request);
    }

    fn get_merge_request(&self, id: &MergeRequestId) -> Option<MergeRequestModel> {
        self.get_merge_request(id)
    }

    fn get_merge_requests_for_branch(&self, branch_id: &BranchId) -> Vec<MergeRequestModel> {
        self.get_merge_requests_for_branch(branch_id)
    }
}
