//! Task domain module
//!
//! This module defines task-related domain types including the Task entity,
/// lifecycle status, and branching strategy detection.
pub mod entities;
pub mod strategy;

// Re-export commonly used items
pub use entities::{Task, TaskStatus};
pub use strategy::{BranchSource, BranchingStrategy, Capability, should_use_branching};
