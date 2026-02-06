//! SQL store with query policy enforcement.

/// Store implementation with `SQLite` backend.
pub mod r#impl;
/// Query policy definitions and enforcement.
pub mod policy;

pub use r#impl::{SqlStore, StoreError};
pub use policy::{PolicyError, PrefixPolicy, QueryPolicy};

#[cfg(test)]
mod integration_tests;
