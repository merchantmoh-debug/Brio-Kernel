//! Virtual file system for sandboxed file operations.

/// File diffing utilities.
pub mod diff;
pub(crate) mod hashing;
/// Session management for isolated file operations.
pub mod manager;
pub use manager::SessionError;
pub(crate) mod policy;
pub mod reflink;
#[cfg(test)]
pub mod tests;
