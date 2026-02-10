//! Parallel Execution Engine for Branch Orchestration
//!
//! Provides concurrent branch execution with resource limits, progress tracking,
//! and support for sequential and parallel agent execution strategies.

pub mod engine;
pub mod strategies;
pub mod tracking;

pub use engine::ParallelExecutionEngine;
pub use tracking::{
    BranchProgress, BranchTreeResult, ExecutionError, ExecutionStatus,
};
