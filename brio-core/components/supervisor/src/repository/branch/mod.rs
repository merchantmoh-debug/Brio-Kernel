//! Branch repository module
//!
//! This module provides branch persistence operations.

pub mod operations;
pub mod traits;
pub mod wit_impl;

// Re-export commonly used items
pub use traits::{BranchRepository, BranchRepositoryError};
pub use wit_impl::WitBranchRepository;
