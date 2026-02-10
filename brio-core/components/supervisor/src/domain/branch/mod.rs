//! Branch domain module
//!
//! This module defines the core branch-related domain types including
//! the Branch entity, lifecycle status, execution configuration, and validation constants.

pub mod entities;
pub mod status;
pub mod validation;

// Re-export commonly used items
pub use entities::{AgentAssignment, BranchConfig, BranchRecord, ExecutionStrategy};
pub use status::BranchStatus;
pub use validation::{MAX_BRANCH_NAME_LEN, MAX_CONCURRENT_BRANCHES, MIN_BRANCH_NAME_LEN};
