//! WebSocket module for JSON Patch broadcasting.

pub mod broadcaster;
pub mod connection;
pub mod handler;
pub mod types;

pub use broadcaster::Broadcaster;
pub use types::{
    BranchEvent, BranchResultSummary, BroadcastMessage, ClientId, ConflictSummary,
    FileChangeSummary, MergeRequestEvent, ProgressUpdate, WsError, WsMessage, WsPatch,
};
