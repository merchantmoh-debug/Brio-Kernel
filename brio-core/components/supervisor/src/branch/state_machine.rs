//! Branch State Machine - Status transitions and validation.
//!
//! This module handles branch state transitions and validation,
//! ensuring branches move through valid state changes.

use tracing::{debug, instrument};

use crate::branch::{BranchError, BranchManager};
use crate::domain::{BranchId, BranchResult, BranchStatus};

impl BranchManager {
    /// Checks if a status is terminal.
    #[cfg(test)]
    pub const fn is_terminal_status(status: BranchStatus) -> bool {
        matches!(
            status,
            BranchStatus::Completed | BranchStatus::Merged | BranchStatus::Failed
        )
    }

    /// Validates status transition.
    pub fn validate_status_transition(
        from: BranchStatus,
        to: BranchStatus,
    ) -> Result<(), BranchError> {
        let valid = match (from, to) {
            // Pending can transition to Active or Failed
            (BranchStatus::Pending, BranchStatus::Active) => true,
            (BranchStatus::Pending, BranchStatus::Failed) => true,
            // Active can transition to Completed, Merging, or Failed
            (BranchStatus::Active, BranchStatus::Completed) => true,
            (BranchStatus::Active, BranchStatus::Merging) => true,
            (BranchStatus::Active, BranchStatus::Failed) => true,
            // Completed can transition to Merging
            (BranchStatus::Completed, BranchStatus::Merging) => true,
            // Merging can transition to Merged or Failed
            (BranchStatus::Merging, BranchStatus::Merged) => true,
            (BranchStatus::Merging, BranchStatus::Failed) => true,
            // Terminal states cannot transition
            (BranchStatus::Merged, _) => false,
            (BranchStatus::Failed, _) => false,
            // All other transitions are invalid
            _ => false,
        };

        if valid {
            Ok(())
        } else {
            Err(BranchError::InvalidStatusTransition { from, to })
        }
    }

    /// Updates the status of a branch with transition validation.
    ///
    /// # Errors
    ///
    /// Returns `BranchError` if:
    /// - Branch not found
    /// - Status transition is invalid
    /// - Repository update fails
    #[instrument(skip(self, id, status))]
    pub fn update_status(&self, id: BranchId, status: BranchStatus) -> Result<(), BranchError> {
        // Get current branch
        let branch = self
            .repository
            .get_branch(id)?
            .ok_or(BranchError::BranchNotFound(id))?;

        // Validate transition
        Self::validate_status_transition(branch.status(), status)?;

        // Persist to repository
        self.repository.update_branch_status(id, status)?;

        debug!("Updated branch {} status to {:?}", id, status);

        Ok(())
    }

    /// Marks a branch as executing (Active status).
    ///
    /// # Errors
    ///
    /// Returns `BranchError` if status transition fails.
    pub fn mark_executing(&self, id: BranchId, _agent_count: usize) -> Result<(), BranchError> {
        self.update_status(id, BranchStatus::Active)
    }

    /// Updates execution progress (placeholder for domain compatibility).
    ///
    /// # Errors
    ///
    /// Returns `BranchError` if branch not found.
    pub fn update_progress(
        &self,
        id: BranchId,
        _active: usize,
        _completed: usize,
    ) -> Result<(), BranchError> {
        // The domain Branch type doesn't track progress details
        // This is a placeholder that validates the branch exists
        let _branch = self
            .repository
            .get_branch(id)?
            .ok_or(BranchError::BranchNotFound(id))?;
        Ok(())
    }

    /// Completes a branch with results.
    ///
    /// # Errors
    ///
    /// Returns `BranchError` if branch not found or transition invalid.
    pub fn complete_branch(&self, id: BranchId, _result: BranchResult) -> Result<(), BranchError> {
        self.update_status(id, BranchStatus::Completed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::BranchStatus;

    #[test]
    fn branch_status_transition_validation() {
        // Valid transitions
        assert!(
            BranchManager::validate_status_transition(BranchStatus::Pending, BranchStatus::Active)
                .is_ok()
        );

        // Invalid transitions
        assert!(
            BranchManager::validate_status_transition(
                BranchStatus::Pending,
                BranchStatus::Completed
            )
            .is_err()
        );
    }

    #[test]
    fn is_terminal_status() {
        assert!(BranchManager::is_terminal_status(BranchStatus::Completed));
        assert!(BranchManager::is_terminal_status(BranchStatus::Merged));
        assert!(BranchManager::is_terminal_status(BranchStatus::Failed));
        assert!(!BranchManager::is_terminal_status(BranchStatus::Pending));
        assert!(!BranchManager::is_terminal_status(BranchStatus::Active));
    }

    #[test]
    fn validate_all_valid_transitions() {
        // Pending -> Active (valid)
        assert!(
            BranchManager::validate_status_transition(BranchStatus::Pending, BranchStatus::Active)
                .is_ok()
        );
        // Pending -> Failed (valid)
        assert!(
            BranchManager::validate_status_transition(BranchStatus::Pending, BranchStatus::Failed)
                .is_ok()
        );
        // Active -> Completed (valid)
        assert!(
            BranchManager::validate_status_transition(
                BranchStatus::Active,
                BranchStatus::Completed
            )
            .is_ok()
        );
        // Active -> Merging (valid)
        assert!(
            BranchManager::validate_status_transition(BranchStatus::Active, BranchStatus::Merging)
                .is_ok()
        );
        // Active -> Failed (valid)
        assert!(
            BranchManager::validate_status_transition(BranchStatus::Active, BranchStatus::Failed)
                .is_ok()
        );
        // Completed -> Merging (valid)
        assert!(
            BranchManager::validate_status_transition(
                BranchStatus::Completed,
                BranchStatus::Merging
            )
            .is_ok()
        );
        // Merging -> Merged (valid)
        assert!(
            BranchManager::validate_status_transition(BranchStatus::Merging, BranchStatus::Merged)
                .is_ok()
        );
        // Merging -> Failed (valid)
        assert!(
            BranchManager::validate_status_transition(BranchStatus::Merging, BranchStatus::Failed)
                .is_ok()
        );
    }

    #[test]
    fn validate_terminal_transitions_fail() {
        // Merged cannot transition to anything
        assert!(
            BranchManager::validate_status_transition(BranchStatus::Merged, BranchStatus::Active)
                .is_err()
        );
        assert!(
            BranchManager::validate_status_transition(BranchStatus::Merged, BranchStatus::Pending)
                .is_err()
        );

        // Failed cannot transition to anything
        assert!(
            BranchManager::validate_status_transition(BranchStatus::Failed, BranchStatus::Active)
                .is_err()
        );
        assert!(
            BranchManager::validate_status_transition(BranchStatus::Failed, BranchStatus::Pending)
                .is_err()
        );
    }
}
