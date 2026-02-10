//! WebSocket module for JSON Patch broadcasting.

pub mod broadcaster;
pub mod connection;
pub mod handler;
pub mod types;

pub use broadcaster::Broadcaster;
pub use types::{
    BranchEvent, BranchResultSummary, BroadcastMessage, ClientId, ClientMessage, ClientResponse,
    ConflictSummary, FileChangeSummary, MergeRequestEvent, ProgressUpdate, ResponseStatus,
    SessionAction, SessionParams, WsError, WsMessage, WsPatch,
};
