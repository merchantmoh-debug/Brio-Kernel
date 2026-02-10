//! Branch domain - Branch lifecycle status
//!
//! This module defines the `BranchStatus` enum and its associated methods.

use serde::{Deserialize, Serialize};

/// Branch lifecycle status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BranchStatus {
    /// Branch is created but not yet active.
    Pending,
    /// Branch is actively being executed.
    Active,
    /// Branch completed successfully.
    Completed,
    /// Branch failed during execution.
    Failed,
    /// Branch is currently being merged.
    Merging,
    /// Branch has been merged.
    Merged,
}

impl BranchStatus {
    /// Checks if this is a terminal status.
    #[must_use]
    pub const fn is_terminal(&self) -> bool {
        matches!(self, Self::Completed | Self::Merged | Self::Failed)
    }

    /// Checks if the branch is in an active state.
    #[must_use]
    pub const fn is_active(&self) -> bool {
        matches!(self, Self::Active | Self::Merging)
    }

    /// Validates that a transition from this status to the target is allowed.
    ///
    /// # Errors
    /// Returns `BranchValidationError::InvalidStatusTransition` if the transition is invalid.
    pub fn validate_transition(
        &self,
        target: &Self,
    ) -> Result<(), crate::domain::BranchValidationError> {
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
            Err(
                crate::domain::BranchValidationError::InvalidStatusTransition {
                    from: *self,
                    to: *target,
                },
            )
        }
    }
}
