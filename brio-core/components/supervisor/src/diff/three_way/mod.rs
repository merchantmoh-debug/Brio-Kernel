//! Three-way merge algorithm implementation.
//!
//! This module provides three-way merge functionality that can detect
//! line-level conflicts by comparing a base version with two branch versions.
//!
//! # Example
//!
//! ```
//! use supervisor::diff::{MyersDiff, three_way_merge, MergeOutcome};
//!
//! let base = "line1\nline2\nline3";
//! let branch_a = "line1\nmodified\nline3";
//! let branch_b = "line1\nline2\nline3\nline4";
//!
//! let result = three_way_merge(base, branch_a, branch_b, &MyersDiff::new()).unwrap();
//! ```

pub mod algorithm;
pub mod conflict;
pub mod outcome;

// Re-export main types
pub use algorithm::{three_way_merge, three_way_merge_with_config};
pub use outcome::{LineConflict, MergeOutcome, ThreeWayConfig, ThreeWayMergeError};

// Internal types are crate-private for encapsulation
