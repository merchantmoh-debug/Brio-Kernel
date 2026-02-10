//! Event base types for WebSocket broadcasting.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;
use thiserror::Error;
use uuid::Uuid;

/// Type of change made to a file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeType {
    /// File was added.
    Added,
    /// File was modified.
    Modified,
    /// File was deleted.
    Deleted,
}

impl fmt::Display for ChangeType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Added => write!(f, "added"),
            Self::Modified => write!(f, "modified"),
            Self::Deleted => write!(f, "deleted"),
        }
    }
}

/// Type of merge conflict.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConflictType {
    /// Content conflict in the file.
    Content,
    /// File was deleted in one branch and modified in another.
    DeleteModify,
    /// File was added in both branches with different content.
    AddAdd,
    /// File was renamed differently in both branches.
    RenameRename,
}

impl fmt::Display for ConflictType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Content => write!(f, "content"),
            Self::DeleteModify => write!(f, "delete_modify"),
            Self::AddAdd => write!(f, "add_add"),
            Self::RenameRename => write!(f, "rename_rename"),
        }
    }
}

/// Type of operation being performed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperationType {
    /// Branch execution operation.
    BranchExecution,
    /// Merge operation.
    Merge,
    /// Rollback operation.
    Rollback,
    /// Sync operation.
    Sync,
}

impl fmt::Display for OperationType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BranchExecution => write!(f, "branch_execution"),
            Self::Merge => write!(f, "merge"),
            Self::Rollback => write!(f, "rollback"),
            Self::Sync => write!(f, "sync"),
        }
    }
}

/// Strategy for executing agents on a branch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionStrategy {
    /// Execute agents sequentially.
    Sequential,
    /// Execute agents in parallel.
    Parallel,
}

impl fmt::Display for ExecutionStrategy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Sequential => write!(f, "sequential"),
            Self::Parallel => write!(f, "parallel"),
        }
    }
}

/// Strategy for merging branches.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MergeStrategy {
    /// Fast-forward merge if possible.
    FastForward,
    /// Create a merge commit.
    MergeCommit,
    /// Squash commits into one.
    Squash,
    /// Rebase and merge.
    Rebase,
}

impl fmt::Display for MergeStrategy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FastForward => write!(f, "fast_forward"),
            Self::MergeCommit => write!(f, "merge_commit"),
            Self::Squash => write!(f, "squash"),
            Self::Rebase => write!(f, "rebase"),
        }
    }
}

/// Metadata common to all branch events.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventMetadata {
    /// Unique identifier for this event.
    pub event_id: String,
    /// Timestamp when the event occurred.
    pub timestamp: DateTime<Utc>,
}

impl EventMetadata {
    /// Creates new event metadata with the current timestamp.
    #[must_use]
    pub fn new() -> Self {
        Self {
            event_id: Uuid::new_v4().to_string(),
            timestamp: Utc::now(),
        }
    }
}

impl Default for EventMetadata {
    fn default() -> Self {
        Self::new()
    }
}

/// Unique identifier for a WebSocket client.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ClientId(Uuid);

impl ClientId {
    /// Generates a new unique client ID.
    #[must_use]
    pub fn generate() -> Self {
        Self(Uuid::new_v4())
    }

    /// Returns the underlying UUID.
    #[must_use]
    pub fn as_uuid(&self) -> Uuid {
        self.0
    }
}

impl fmt::Display for ClientId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A WebSocket patch message containing a JSON Patch.
#[derive(Debug, Clone)]
pub struct WsPatch {
    inner: json_patch::Patch,
}

impl WsPatch {
    /// Creates a new WebSocket patch.
    ///
    /// # Arguments
    ///
    /// * `patch` - The JSON Patch to wrap.
    #[must_use]
    pub fn new(patch: json_patch::Patch) -> Self {
        Self { inner: patch }
    }

    /// Returns a reference to the underlying JSON Patch.
    #[must_use]
    pub fn inner(&self) -> &json_patch::Patch {
        &self.inner
    }

    /// Serializes the patch to a JSON string.
    ///
    /// # Errors
    ///
    /// Returns an error if the patch cannot be serialized to JSON.
    pub fn to_json(&self) -> Result<String, WsError> {
        serde_json::to_string(&self.inner).map_err(WsError::Serialization)
    }
}

/// Messages that can be broadcast to WebSocket clients.
#[derive(Debug, Clone)]
pub enum BroadcastMessage {
    /// A JSON Patch message.
    Patch(Box<WsPatch>),
    /// Server shutdown signal.
    Shutdown,
    /// A structured WebSocket message.
    Message(WsMessage),
}

impl BroadcastMessage {
    /// Converts the message to a WebSocket frame payload.
    ///
    /// # Errors
    ///
    /// Returns an error if a patch message cannot be serialized to JSON.
    pub fn to_frame_payload(&self) -> Result<String, WsError> {
        match self {
            Self::Patch(patch) => patch.to_json(),
            Self::Shutdown => Ok(r#"{"type":"shutdown"}"#.to_string()),
            Self::Message(msg) => serde_json::to_string(msg).map_err(WsError::Serialization),
        }
    }
}

/// WebSocket message types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WsMessage {
    /// Branch lifecycle event
    BranchEvent(super::branch::BranchEvent),
    /// Merge request event
    MergeRequestEvent(super::merge::MergeRequestEvent),
    /// Progress update for long-running operations
    ProgressUpdate(super::metrics::ProgressUpdate),
}

/// Client message types received from WebSocket clients
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMessage {
    /// Task submission message
    Task {
        /// Task content/description
        content: String,
    },
    /// Session management message
    Session {
        /// Session action to perform
        action: SessionAction,
        /// Additional parameters for the action
        #[serde(flatten)]
        params: SessionParams,
    },
    /// SQL query message
    Query {
        /// SQL query string (SELECT only)
        sql: String,
    },
}

/// Session action types
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionAction {
    /// Begin a new session
    Begin,
    /// Commit an existing session
    Commit,
    /// Rollback an existing session
    Rollback,
}

/// Session parameters for different actions
#[derive(Debug, Clone, Deserialize, Default)]
pub struct SessionParams {
    /// Base path for the session (required for Begin)
    pub base_path: Option<String>,
    /// Session ID (required for Commit and Rollback)
    pub session_id: Option<String>,
}

/// Response to client messages
#[derive(Debug, Clone, Serialize)]
pub struct ClientResponse {
    /// Response status (success or error)
    pub status: ResponseStatus,
    /// Response data (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
    /// Error message (optional, only present on errors)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// Response status for client messages
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ResponseStatus {
    /// Operation completed successfully
    Success,
    /// Operation failed
    Error,
}

impl ClientResponse {
    /// Create a successful response with optional data
    #[must_use]
    pub fn success(data: Option<serde_json::Value>) -> Self {
        Self {
            status: ResponseStatus::Success,
            data,
            message: None,
        }
    }

    /// Create an error response with a message
    pub fn error(message: impl Into<String>) -> Self {
        Self {
            status: ResponseStatus::Error,
            data: None,
            message: Some(message.into()),
        }
    }
}

/// Errors that can occur in WebSocket operations.
#[derive(Debug, Error)]
pub enum WsError {
    /// WebSocket connection error.
    #[error("WebSocket connection error: {0}")]
    AxumWs(#[from] axum::Error),

    /// JSON serialization error.
    #[error("Serialization error: {0}")]
    Serialization(#[source] serde_json::Error),

    /// The broadcast channel was closed.
    #[error("Broadcast channel closed")]
    ChannelClosed,

    /// Client disconnected from the connection.
    #[error("Connection closed by client")]
    ClientDisconnected,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn client_id_is_unique() {
        let id1 = ClientId::generate();
        let id2 = ClientId::generate();
        assert_ne!(id1, id2);
    }

    #[test]
    fn client_id_display() {
        let id = ClientId::generate();
        let display = format!("{id}");
        assert!(!display.is_empty());
    }

    #[test]
    fn broadcast_message_shutdown_serializes() -> Result<(), WsError> {
        let msg = BroadcastMessage::Shutdown;
        let payload = msg.to_frame_payload()?;
        assert_eq!(payload, r#"{"type":"shutdown"}"#);
        Ok(())
    }
}
