//! Branch management tools for agents.
//!
//! This module provides tools that allow agents to create and manage branches
//! autonomously. The tools use a callback-based approach to avoid circular
//! dependencies between agent-sdk and supervisor.

pub mod callbacks;
pub mod operations;

pub use callbacks::{
    BranchCreationCallback, BranchCreationConfig, BranchCreationResult, BranchId, BranchInfo,
    BranchListCallback, BranchToolError,
};
pub use operations::{CreateBranchTool, ListBranchesTool, parse_branch_config};
