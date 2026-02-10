//! Domain types for WebSocket broadcasting.

pub mod branch;
pub mod events;
pub mod merge;
pub mod metrics;

pub use branch::{BranchEvent, BranchId, BranchResultSummary, ConflictSummary, FileChangeSummary};
pub use events::{
    BroadcastMessage, ChangeType, ClientId, ClientMessage, ClientResponse, ConflictType,
    EventMetadata, ExecutionStrategy, MergeStrategy, OperationType, ResponseStatus, SessionAction,
    SessionParams, WsError, WsMessage, WsPatch,
};
pub use merge::MergeRequestEvent;
pub use metrics::{ProgressUpdate, ProgressUpdateError};
