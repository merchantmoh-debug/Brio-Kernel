//! Merge strategies for combining changes from multiple branches.
//!
//! This module provides different strategies for merging file changes from multiple
//! branches, including conflict detection and resolution approaches.

pub mod conflict;
pub mod strategies;

// Re-export conflict types
pub use conflict::{
    BranchResult, Conflict, DiffError, FileChange, MergeError, MergeId, MergeResult,
    changes_conflict, detect_conflicts, is_binary_file,
};

// Re-export strategy types
pub use strategies::{MergeStrategy, MergeStrategyRegistry, validate_branch_count};

// Re-export specific strategies
pub use strategies::three_way::{
    OursStrategy, TheirsStrategy, ThreeWayMergeConfig, ThreeWayStrategy,
};
pub use strategies::union::UnionStrategy;
