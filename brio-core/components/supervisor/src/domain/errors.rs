//! Domain errors - Error types for validation and parsing failures
//!
//! This module defines error types used throughout the domain layer
//! for validation failures and parsing errors.

use core::fmt;

/// Error type for domain validation failures.
#[derive(Debug, Clone)]
pub enum ValidationError {
    /// `AgentId` cannot be empty.
    EmptyAgentId,
    /// Task content cannot be empty.
    EmptyTaskContent,
    /// Branch name cannot be empty.
    EmptyBranchName,
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyAgentId => write!(f, "AgentId cannot be empty"),
            Self::EmptyTaskContent => write!(f, "Task content cannot be empty"),
            Self::EmptyBranchName => write!(f, "Branch name cannot be empty"),
        }
    }
}

impl std::error::Error for ValidationError {}

/// Error type for branch validation failures.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum BranchValidationError {
    /// Branch name is empty or too short.
    InvalidNameLength {
        /// The actual length of the name.
        len: usize,
        /// The minimum allowed length.
        min: usize,
        /// The maximum allowed length.
        max: usize,
    },
    /// Session ID is empty.
    EmptySessionId,
    /// Maximum concurrent branches exceeded.
    MaxConcurrentBranchesExceeded {
        /// The number of branches requested.
        requested: usize,
        /// The maximum allowed branches.
        max: usize,
    },
    /// Invalid execution strategy configuration.
    InvalidExecutionStrategy {
        /// The reason the strategy is invalid.
        reason: String,
    },
    /// Cannot transition from current status.
    InvalidStatusTransition {
        /// The current status.
        from: super::BranchStatus,
        /// The target status.
        to: super::BranchStatus,
    },
    /// Agent assignment is invalid.
    InvalidAgentAssignment {
        /// The reason the assignment is invalid.
        reason: String,
    },
    /// Invalid timestamp value.
    InvalidTimestamp {
        /// The field name containing the invalid timestamp.
        field: String,
        /// The invalid timestamp value.
        value: i64,
    },
}

impl fmt::Display for BranchValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidNameLength { len, min, max } => {
                write!(
                    f,
                    "Branch name length {len} is outside valid range [{min}-{max}]"
                )
            }
            Self::EmptySessionId => write!(f, "Session ID cannot be empty"),
            Self::MaxConcurrentBranchesExceeded { requested, max } => {
                write!(
                    f,
                    "Requested {requested} concurrent branches, but maximum is {max}"
                )
            }
            Self::InvalidExecutionStrategy { reason } => {
                write!(f, "Invalid execution strategy: {reason}")
            }
            Self::InvalidStatusTransition { from, to } => {
                write!(f, "Cannot transition from {from:?} to {to:?}")
            }
            Self::InvalidAgentAssignment { reason } => {
                write!(f, "Invalid agent assignment: {reason}")
            }
            Self::InvalidTimestamp { field, value } => {
                write!(f, "Invalid timestamp for field '{field}': {value}")
            }
        }
    }
}

impl std::error::Error for BranchValidationError {}

impl From<ValidationError> for BranchValidationError {
    fn from(err: ValidationError) -> Self {
        match err {
            ValidationError::EmptyAgentId => Self::InvalidAgentAssignment {
                reason: "Agent ID cannot be empty".to_string(),
            },
            ValidationError::EmptyTaskContent => Self::InvalidAgentAssignment {
                reason: "Task override cannot be empty".to_string(),
            },
            ValidationError::EmptyBranchName => Self::InvalidNameLength {
                len: 0,
                min: super::MIN_BRANCH_NAME_LEN,
                max: super::MAX_BRANCH_NAME_LEN,
            },
        }
    }
}

/// Error when parsing an unknown status string.
#[derive(Debug, Clone)]
pub struct ParseStatusError(pub String);

impl fmt::Display for ParseStatusError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Unknown task status: '{}'", self.0)
    }
}

impl std::error::Error for ParseStatusError {}
