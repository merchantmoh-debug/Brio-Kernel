//! Merge domain - Merge operations, conflicts, and results
//!
//! This module defines merge-related domain types including merge requests,
//! conflict detection, and merge results.
//!
//! # Example
//!
//! ```
//! use supervisor::domain::merge::{ChangeType, FileChange, MergeResult, MergeRequest};
//! use std::path::PathBuf;
//!
//! let change = FileChange::new(
//!     PathBuf::from("src/main.rs"),
//!     ChangeType::Modified,
//!     None,
//! );
//! ```

pub mod change;
pub mod conflict;
pub mod result;

// Re-export change types
pub use change::{
    AgentResult, BranchResult, ChangeType, ExecutionMetrics, FileChange, StagedChange,
};

// Re-export conflict types
pub use conflict::{Conflict, ConflictType};

// Re-export result types
pub use result::{MergeRequest, MergeRequestStatus, MergeResult, MergeStatus};
