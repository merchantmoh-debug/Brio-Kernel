//! Domain types for WebSocket broadcasting.

use std::fmt;
use thiserror::Error;
use uuid::Uuid;

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
    fn broadcast_message_shutdown_serializes() {
        let msg = BroadcastMessage::Shutdown;
        let payload = msg.to_frame_payload().unwrap();
        assert_eq!(payload, r#"{"type":"shutdown"}"#);
    }
}
