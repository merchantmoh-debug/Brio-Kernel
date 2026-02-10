//! Branch Event Broadcasting
//!
//! Provides WebSocket event broadcasting for branch lifecycle events.

pub mod broadcaster;
pub mod handlers;

pub use broadcaster::BranchEventBroadcaster;
pub use handlers::{BranchEventHandlers, MergeEventHandlers};
