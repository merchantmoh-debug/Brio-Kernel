//! Branch manager for the Brio kernel.
//!
//! This module provides the BranchManager and related domain types for
//! managing branches in the Brio system. It is separate from the API layer
//! to avoid circular dependencies.

pub mod core;
pub mod storage;
pub mod types;

// Re-export primary types for convenience
pub use core::BranchManager;
pub use storage::{BranchStorage, BranchStoragePort, MergeRequestStoragePort};
pub use types::{
    AgentAssignment, Branch, BranchConfig, BranchError, BranchId, BranchStatus, ExecutionStrategy,
    MergeRequestId, MergeRequestModel, MergeRequestStatus,
};
