pub mod events;
pub mod grpc;
pub mod remote;
pub mod service;
pub mod types;

pub use service::MeshService;
pub use types::*;

use tokio::sync::oneshot;

#[derive(Debug, Clone)]
pub enum Payload {
    Json(Box<String>),
    Binary(Box<Vec<u8>>),
}

pub struct MeshMessage {
    pub target: String,
    pub method: String,
    pub payload: Payload,
    // The return channel.
    // We start simple: A result containing a Payload or an Error string.
    pub reply_tx: oneshot::Sender<Result<Payload, String>>,
}
