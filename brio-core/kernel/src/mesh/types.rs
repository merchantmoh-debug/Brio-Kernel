use serde::{Deserialize, Serialize};
use std::fmt;

/// Unique identifier for a kernel node in the cluster
#[derive(Debug, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct NodeId(pub String);

impl NodeId {
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }
}

impl Default for NodeId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for NodeId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

/// Address where a node acts as a gRPC server
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NodeAddress(pub String);

impl fmt::Display for NodeAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Metadata about a registered node
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeInfo {
    pub id: NodeId,
    pub address: NodeAddress,
    pub capabilities: Vec<String>,
    pub last_seen: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeshConfig {
    pub node_id: String,
    pub listen_address: String,
    pub bootstrap_nodes: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_id_creation() {
        let id1 = NodeId::new();
        let id2 = NodeId::new();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_node_id_display() {
        let id = NodeId("test-node".to_string());
        assert_eq!(id.to_string(), "test-node");
    }

    #[test]
    fn test_serialization() {
        let info = NodeInfo {
            id: NodeId("node-1".to_string()),
            address: NodeAddress("127.0.0.1:8080".to_string()),
            capabilities: vec!["mesh".to_string()],
            last_seen: 100,
        };

        let json = serde_json::to_string(&info).unwrap();
        let deserialized: NodeInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, info.id);
        assert_eq!(deserialized.address, info.address);
    }
}
