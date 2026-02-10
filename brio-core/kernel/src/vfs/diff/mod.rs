//! File diff computation and application for session management.
//!
//! This module provides utilities for detecting changes between directories
//! and applying them atomically using a staging approach.

pub mod apply;
pub mod compute;

// Re-export primary types for convenience
pub use apply::{apply_changes, apply_single_change};
pub use compute::{FileChange, compute_diff};
