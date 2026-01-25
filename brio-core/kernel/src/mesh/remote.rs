use anyhow::{Result, anyhow};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use tonic::transport::Channel;

use crate::mesh::grpc::mesh_transport_client::MeshTransportClient;
use crate::mesh::types::{NodeAddress, NodeId, NodeInfo};
use crate::mesh::{MeshMessage, Payload};

/// Router for dispatching mesh calls to remote nodes via gRPC.
/// Handles connection pooling and payload serialization.
#[derive(Clone)]
pub struct RemoteRouter {
    registry: Arc<RwLock<NodeRegistry>>,
    clients: Arc<RwLock<HashMap<NodeId, MeshTransportClient<Channel>>>>,
}

impl Default for RemoteRouter {
    fn default() -> Self {
        Self::new()
    }
}

impl RemoteRouter {
    pub fn new() -> Self {
        Self {
            registry: Arc::new(RwLock::new(NodeRegistry::new())),
            clients: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn register_node(&self, info: NodeInfo) {
        let mut registry = self.registry.write().expect("Registry lock poisoned");
        registry.register(info);
    }

    pub fn get_node_address(&self, node_id: &NodeId) -> Option<NodeAddress> {
        let registry = self.registry.read().expect("Registry lock poisoned");
        registry.get(node_id).map(|info| info.address.clone())
    }

    pub async fn send(&self, target_node: &NodeId, message: MeshMessage) -> Result<Payload> {
        let client = self.get_or_connect(target_node).await?;

        let request = tonic::Request::new(crate::mesh::grpc::MeshRequest {
            target: message.target,
            method: message.method,
            payload: Some(match message.payload {
                Payload::Json(s) => crate::mesh::grpc::mesh_request::Payload::Json(s),
                Payload::Binary(b) => crate::mesh::grpc::mesh_request::Payload::Binary(b),
            }),
        });

        // We need a mutable client for the call, so we clone the channel which is cheap
        let mut client = client.clone();

        let response = client.call(request).await?.into_inner();

        match response.payload {
            Some(crate::mesh::grpc::mesh_response::Payload::Json(s)) => Ok(Payload::Json(s)),
            Some(crate::mesh::grpc::mesh_response::Payload::Binary(b)) => Ok(Payload::Binary(b)),
            Some(crate::mesh::grpc::mesh_response::Payload::Error(e)) => {
                Err(anyhow!("Remote error: {}", e))
            }
            None => Err(anyhow!("Empty response payload")),
        }
    }

    async fn get_or_connect(&self, node_id: &NodeId) -> Result<MeshTransportClient<Channel>> {
        // Fast path: check if connected
        {
            let clients = self.clients.read().expect("Clients lock poisoned");
            if let Some(client) = clients.get(node_id) {
                return Ok(client.clone());
            }
        }

        // Slow path: connect
        let address = self
            .get_node_address(node_id)
            .ok_or_else(|| anyhow!("Node {} not found in registry", node_id))?;

        // Format as http URL for tonic
        let url = format!("http://{}", address); // Assuming HTTP/2 over cleartext for now
        let endpoint = Channel::from_shared(url)?;
        let channel = endpoint.connect().await?;
        let client = MeshTransportClient::new(channel);

        {
            let mut clients = self.clients.write().expect("Clients lock poisoned");
            clients.insert(node_id.clone(), client.clone());
        }

        Ok(client)
    }
}

pub struct NodeRegistry {
    nodes: HashMap<NodeId, NodeInfo>,
}

impl Default for NodeRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl NodeRegistry {
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
        }
    }

    pub fn register(&mut self, info: NodeInfo) {
        self.nodes.insert(info.id.clone(), info);
    }

    pub fn get(&self, id: &NodeId) -> Option<&NodeInfo> {
        self.nodes.get(id)
    }

    pub fn list(&self) -> Vec<NodeInfo> {
        self.nodes.values().cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_operations() {
        let mut registry = NodeRegistry::new();
        let id = NodeId::new();
        let info = NodeInfo {
            id: id.clone(),
            address: NodeAddress("127.0.0.1:8080".to_string()),
            capabilities: vec![],
            last_seen: 0,
        };

        registry.register(info.clone());
        assert!(registry.get(&id).is_some());

        let list = registry.list();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, id);
    }
}
