//! Session manager for isolated file system operations.
//!
//! This module manages temporary working directories for agents, providing
//! copy-on-write isolation through reflinks and atomic commit/rollback semantics.

pub mod isolation;
pub mod session;
pub mod types;

// Re-export primary types for convenience
pub use isolation::IsolationOps;
pub use session::SessionManager;
pub use types::SessionError;
