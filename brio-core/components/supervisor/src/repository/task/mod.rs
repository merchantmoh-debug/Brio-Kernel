//! Task repository module
//!
//! This module provides task persistence operations.

pub mod traits;
pub mod wit_impl;

// Re-export commonly used items
pub use traits::TaskRepository;
pub use wit_impl::WitTaskRepository;
