//! Branch Repository - Traits and Error Types
//!
//! This module defines the `BranchRepository` trait and related error types.

use crate::domain::{BranchId, BranchRecord, BranchStatus, MergeRequest};
use crate::merge::MergeId;
use crate::repository::RepositoryError;

/// Errors specific to branch repository operations.
#[derive(Debug)]
pub enum BranchRepositoryError {
    /// SQL query or execution failed.
    SqlError(String),
    /// Failed to parse data from database.
    ParseError(String),
    /// Branch not found for given ID.
    BranchNotFound(BranchId),
    /// Merge request not found for given ID.
    MergeRequestNotFound(MergeId),
    /// Invalid UUID format.
    InvalidUuid(String),
    /// JSON serialization/deserialization failed.
    JsonError(String),
}

impl core::fmt::Display for BranchRepositoryError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::SqlError(msg) => write!(f, "SQL error: {msg}"),
            Self::ParseError(msg) => write!(f, "Parse error: {msg}"),
            Self::BranchNotFound(id) => write!(f, "Branch not found: {id}"),
            Self::MergeRequestNotFound(id) => write!(f, "Merge request not found: {id}"),
            Self::InvalidUuid(msg) => write!(f, "Invalid UUID: {msg}"),
            Self::JsonError(msg) => write!(f, "JSON error: {msg}"),
        }
    }
}

impl std::error::Error for BranchRepositoryError {}

impl From<RepositoryError> for BranchRepositoryError {
    fn from(e: RepositoryError) -> Self {
        match e {
            RepositoryError::SqlError(msg) => Self::SqlError(msg),
            RepositoryError::ParseError(msg) => Self::ParseError(msg),
            RepositoryError::NotFound(_) => Self::SqlError(e.to_string()),
        }
    }
}

impl From<serde_json::Error> for BranchRepositoryError {
    fn from(e: serde_json::Error) -> Self {
        Self::JsonError(e.to_string())
    }
}

impl From<uuid::Error> for BranchRepositoryError {
    fn from(e: uuid::Error) -> Self {
        Self::InvalidUuid(e.to_string())
    }
}

/// Contract for branch persistence operations.
///
/// This trait abstracts the database layer for branch management, enabling:
/// - Unit testing with mock implementations
/// - Swapping storage backends without changing business logic
/// - Recovery of branch state after kernel restarts
pub trait BranchRepository: Send + Sync {
    /// Creates a new branch in the repository.
    ///
    /// # Errors
    /// Returns `BranchRepositoryError` if the operation fails.
    fn create_branch(&self, branch: &BranchRecord) -> Result<BranchId, BranchRepositoryError>;

    /// Fetches a branch by its ID.
    ///
    /// # Errors
    /// Returns `BranchRepositoryError` if the query fails.
    fn get_branch(&self, id: BranchId) -> Result<Option<BranchRecord>, BranchRepositoryError>;

    /// Updates the status of a branch.
    ///
    /// # Errors
    /// Returns `BranchRepositoryError` if the update fails.
    fn update_branch_status(
        &self,
        id: BranchId,
        status: BranchStatus,
    ) -> Result<(), BranchRepositoryError>;

    /// Lists all active branches (pending, active, merging).
    ///
    /// # Errors
    /// Returns `BranchRepositoryError` if the query fails.
    fn list_active_branches(&self) -> Result<Vec<BranchRecord>, BranchRepositoryError>;

    /// Lists all branches that have a specific parent.
    ///
    /// # Errors
    /// Returns `BranchRepositoryError` if the query fails.
    fn list_branches_by_parent(
        &self,
        parent_id: BranchId,
    ) -> Result<Vec<BranchRecord>, BranchRepositoryError>;

    /// Deletes a branch and all its associated data.
    ///
    /// # Errors
    /// Returns `BranchRepositoryError` if the deletion fails.
    fn delete_branch(&self, id: BranchId) -> Result<(), BranchRepositoryError>;

    /// Creates a new merge request in the queue.
    ///
    /// # Errors
    /// Returns `BranchRepositoryError` if the creation fails.
    fn create_merge_request(
        &self,
        branch_id: BranchId,
        parent_id: Option<BranchId>,
        strategy: &str,
    ) -> Result<MergeId, BranchRepositoryError>;

    /// Gets a merge request by its ID.
    ///
    /// # Errors
    /// Returns `BranchRepositoryError` if the query fails.
    fn get_merge_request(
        &self,
        merge_id: MergeId,
    ) -> Result<Option<MergeRequest>, BranchRepositoryError>;

    /// Updates a merge request's status and staging information.
    ///
    /// # Errors
    /// Returns `BranchRepositoryError` if the update fails.
    fn update_merge_request(
        &self,
        merge_request: &MergeRequest,
    ) -> Result<(), BranchRepositoryError>;

    /// Approves a merge request.
    ///
    /// # Errors
    /// Returns `BranchRepositoryError` if the approval fails.
    fn approve_merge(&self, merge_id: MergeId, approver: &str)
    -> Result<(), BranchRepositoryError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::BranchId;

    #[test]
    fn branch_repository_error_display() {
        let id = BranchId::new();
        let err = BranchRepositoryError::BranchNotFound(id);
        assert!(err.to_string().contains("not found"));
    }
}
