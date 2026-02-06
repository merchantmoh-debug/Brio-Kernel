//! Distributed mesh networking for inter-node communication.

/// Event bus for pub/sub messaging between nodes.
pub mod events;
/// gRPC transport implementation.
pub mod grpc;
/// Remote node management and registry.
pub mod remote;
/// Mesh service implementation.
pub mod service;
/// Core types for mesh networking.
pub mod types;

pub use service::MeshService;
pub use types::*;

use tokio::sync::oneshot;

/// Payload type for mesh messages.
#[derive(Debug, Clone)]
pub enum Payload {
    /// JSON payload.
    Json(Box<String>),
    /// Binary payload.
    Binary(Box<Vec<u8>>),
}

/// A message sent through the mesh network.
pub struct MeshMessage {
    /// Target node or service.
    pub target: String,
    /// Method to invoke.
    pub method: String,
    /// Message payload.
    pub payload: Payload,
    /// Channel for receiving the response.
    pub reply_tx: oneshot::Sender<Result<Payload, String>>,
}
