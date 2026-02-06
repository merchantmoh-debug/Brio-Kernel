use std::sync::Arc;
use tonic::{Request, Response, Status};

use crate::host::BrioHostState;
use crate::mesh::Payload;
use crate::mesh::grpc::{
    HeartbeatRequest, HeartbeatResponse, MeshRequest, MeshResponse,
    mesh_request::Payload as RequestPayload, mesh_response::Payload as ResponsePayload,
    mesh_transport_server::MeshTransport,
};
use crate::mesh::types::NodeId;

/// gRPC Service Implementation for `MeshTransport`.
/// Handles incoming RPC calls and routes them to local components via `BrioHostState`.
pub struct MeshService {
    host: Arc<BrioHostState>,
    node_id: NodeId,
}

impl MeshService {
    #[must_use] 
    pub fn new(host: Arc<BrioHostState>, node_id: NodeId) -> Self {
        Self { host, node_id }
    }
}

#[tonic::async_trait]
impl MeshTransport for MeshService {
    async fn call(&self, request: Request<MeshRequest>) -> Result<Response<MeshResponse>, Status> {
        let req = request.into_inner();

        let payload = match req.payload {
            Some(RequestPayload::Json(s)) => Payload::Json(Box::new(s)),
            Some(RequestPayload::Binary(b)) => Payload::Binary(Box::new(b)),
            None => return Err(Status::invalid_argument("Missing payload")),
        };

        // Execute call against local host
        // Note: We use the raw component ID as the target, assuming incoming requests are for this node
        match self.host.mesh_call(&req.target, &req.method, payload).await {
            Ok(Payload::Json(s)) => Ok(Response::new(MeshResponse {
                payload: Some(ResponsePayload::Json(*s)),
            })),
            Ok(Payload::Binary(b)) => Ok(Response::new(MeshResponse {
                payload: Some(ResponsePayload::Binary(*b)),
            })),
            Err(e) => Ok(Response::new(MeshResponse {
                payload: Some(ResponsePayload::Error(e.to_string())),
            })),
        }
    }

    async fn heartbeat(
        &self,
        _request: Request<HeartbeatRequest>,
    ) -> Result<Response<HeartbeatResponse>, Status> {
        let timestamp_secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        // Safe cast: Unix timestamps won't exceed i64 range until year 292 billion
        #[expect(clippy::cast_possible_wrap)]
        let timestamp = timestamp_secs as i64;
        Ok(Response::new(HeartbeatResponse {
            node_id: self.node_id.to_string(),
            ready: true,
            timestamp,
        }))
    }
}
