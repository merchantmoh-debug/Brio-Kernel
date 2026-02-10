//! Mesh configuration and registry types for mesh networking.

use serde::{Deserialize, Serialize};

use super::addressing::NodeAddress;
use super::node::{NodeId, ValidationError};

/// Configuration for mesh networking with validated types.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeshConfig {
    /// Unique identifier for this node.
    pub node_id: NodeId,
    /// Address to listen on for incoming connections.
    pub listen_address: NodeAddress,
    /// List of bootstrap node addresses to connect to.
    pub bootstrap_nodes: Vec<NodeAddress>,
}

impl MeshConfig {
    /// Creates a new `MeshConfig` with validation.
    ///
    /// # Errors
    /// Returns `ValidationError` if validation fails.
    pub fn new(
        node_id: NodeId,
        listen_address: NodeAddress,
        bootstrap_nodes: Vec<NodeAddress>,
    ) -> Result<Self, ValidationError> {
        Ok(Self {
            node_id,
            listen_address,
            bootstrap_nodes,
        })
    }

    /// Returns a builder for constructing `MeshConfig` with validation.
    #[must_use]
    pub fn builder() -> MeshConfigBuilder {
        MeshConfigBuilder::default()
    }
}

/// Builder for constructing `MeshConfig` instances with validation.
#[derive(Debug, Default)]
pub struct MeshConfigBuilder {
    node_id: Option<NodeId>,
    listen_address: Option<NodeAddress>,
    bootstrap_nodes: Vec<NodeAddress>,
}

impl MeshConfigBuilder {
    /// Sets the node ID.
    #[must_use]
    pub fn node_id(mut self, node_id: NodeId) -> Self {
        self.node_id = Some(node_id);
        self
    }

    /// Sets the listen address.
    #[must_use]
    pub fn listen_address(mut self, listen_address: NodeAddress) -> Self {
        self.listen_address = Some(listen_address);
        self
    }

    /// Adds a bootstrap node address.
    #[must_use]
    pub fn bootstrap_node(mut self, node_address: NodeAddress) -> Self {
        self.bootstrap_nodes.push(node_address);
        self
    }

    /// Sets all bootstrap node addresses.
    #[must_use]
    pub fn bootstrap_nodes(mut self, bootstrap_nodes: Vec<NodeAddress>) -> Self {
        self.bootstrap_nodes = bootstrap_nodes;
        self
    }

    /// Builds the `MeshConfig` instance.
    ///
    /// # Errors
    /// Returns `ValidationError` if required fields are missing.
    pub fn build(self) -> Result<MeshConfig, ValidationError> {
        let node_id = self.node_id.ok_or(ValidationError::EmptyId)?;
        let listen_address = self.listen_address.ok_or(ValidationError::EmptyAddress)?;

        MeshConfig::new(node_id, listen_address, self.bootstrap_nodes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mesh_config_new() {
        let config = MeshConfig::new(
            NodeId::try_from_str("node-1").unwrap(),
            NodeAddress::new("0.0.0.0:8080").unwrap(),
            vec![NodeAddress::new("127.0.0.1:9090").unwrap()],
        )
        .unwrap();

        assert_eq!(config.node_id.as_str(), "node-1");
        assert_eq!(config.listen_address.as_str(), "0.0.0.0:8080");
        assert_eq!(config.bootstrap_nodes.len(), 1);
        assert_eq!(config.bootstrap_nodes[0].as_str(), "127.0.0.1:9090");
    }

    #[test]
    fn test_mesh_config_builder() {
        let config = MeshConfig::builder()
            .node_id(NodeId::try_from_str("node-1").unwrap())
            .listen_address(NodeAddress::new("0.0.0.0:8080").unwrap())
            .bootstrap_node(NodeAddress::new("127.0.0.1:9090").unwrap())
            .bootstrap_node(NodeAddress::new("127.0.0.1:9091").unwrap())
            .build()
            .unwrap();

        assert_eq!(config.node_id.as_str(), "node-1");
        assert_eq!(config.listen_address.as_str(), "0.0.0.0:8080");
        assert_eq!(config.bootstrap_nodes.len(), 2);
    }

    #[test]
    fn test_mesh_config_builder_missing_node_id() {
        let result = MeshConfig::builder()
            .listen_address(NodeAddress::new("0.0.0.0:8080").unwrap())
            .build();
        assert!(matches!(result, Err(ValidationError::EmptyId)));
    }

    #[test]
    fn test_mesh_config_builder_missing_address() {
        let result = MeshConfig::builder()
            .node_id(NodeId::try_from_str("node-1").unwrap())
            .build();
        assert!(matches!(result, Err(ValidationError::EmptyAddress)));
    }

    #[test]
    fn test_mesh_config_serialization() {
        let config = MeshConfig::new(
            NodeId::try_from_str("node-1").unwrap(),
            NodeAddress::new("0.0.0.0:8080").unwrap(),
            vec![NodeAddress::new("127.0.0.1:9090").unwrap()],
        )
        .unwrap();

        let json = serde_json::to_string(&config).unwrap();
        let deserialized: MeshConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.node_id, config.node_id);
        assert_eq!(deserialized.listen_address, config.listen_address);
        assert_eq!(
            deserialized.bootstrap_nodes.len(),
            config.bootstrap_nodes.len()
        );
    }
}
