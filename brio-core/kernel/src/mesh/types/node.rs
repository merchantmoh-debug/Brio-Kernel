//! Node identification and information types for mesh networking.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Error type for validation failures
#[derive(Debug, Clone, PartialEq)]
pub enum ValidationError {
    /// The node ID is empty or contains only whitespace.
    EmptyId,
    /// The node address is empty or contains only whitespace.
    EmptyAddress,
    /// The address format is invalid with a descriptive message.
    InvalidAddressFormat(String),
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ValidationError::EmptyId => write!(f, "Node ID cannot be empty"),
            ValidationError::EmptyAddress => write!(f, "Node address cannot be empty"),
            ValidationError::InvalidAddressFormat(msg) => {
                write!(f, "Invalid address format: {msg}")
            }
        }
    }
}

impl std::error::Error for ValidationError {}

/// Unique identifier for a kernel node in the cluster
#[derive(Debug, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct NodeId(String);

impl NodeId {
    /// Creates a new unique node ID with a generated UUID.
    #[must_use]
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }

    /// Creates a `NodeId` from a string, validating it is non-empty.
    ///
    /// # Errors
    /// Returns `ValidationError::EmptyId` if the provided string is empty.
    pub fn from_str(s: &str) -> Result<Self, ValidationError> {
        if s.trim().is_empty() {
            return Err(ValidationError::EmptyId);
        }
        Ok(Self(s.to_string()))
    }

    /// Returns the string representation of this `NodeId`.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Consumes the `NodeId` and returns the inner String.
    #[must_use]
    pub fn into_string(self) -> String {
        self.0
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

/// Metadata about a registered node
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeInfo {
    /// Unique identifier for the node.
    id: NodeId,
    /// Network address of the node.
    address: super::addressing::NodeAddress,
    /// Capabilities advertised by the node.
    capabilities: Vec<String>,
    /// Unix timestamp of last heartbeat.
    last_seen: u64,
}

impl NodeInfo {
    /// Creates a new `NodeInfo` with validation.
    ///
    /// # Errors
    /// Returns `ValidationError` if validation fails (e.g., empty capabilities list).
    pub fn new(
        id: NodeId,
        address: super::addressing::NodeAddress,
        capabilities: Vec<String>,
        last_seen: u64,
    ) -> Result<Self, ValidationError> {
        Ok(Self {
            id,
            address,
            capabilities,
            last_seen,
        })
    }

    /// Returns the node ID.
    #[must_use]
    pub fn id(&self) -> &NodeId {
        &self.id
    }

    /// Returns the node address.
    #[must_use]
    pub fn address(&self) -> &super::addressing::NodeAddress {
        &self.address
    }

    /// Returns the node capabilities.
    #[must_use]
    pub fn capabilities(&self) -> &[String] {
        &self.capabilities
    }

    /// Returns the last seen timestamp.
    #[must_use]
    pub fn last_seen(&self) -> u64 {
        self.last_seen
    }

    /// Updates the last seen timestamp.
    pub fn update_last_seen(&mut self, timestamp: u64) {
        self.last_seen = timestamp;
    }

    /// Adds a capability to the node.
    pub fn add_capability(&mut self, capability: String) {
        self.capabilities.push(capability);
    }

    /// Returns a builder for constructing `NodeInfo` with validation.
    #[must_use]
    pub fn builder() -> NodeInfoBuilder {
        NodeInfoBuilder::default()
    }
}

/// Builder for constructing `NodeInfo` instances with validation.
#[derive(Debug, Default)]
pub struct NodeInfoBuilder {
    id: Option<NodeId>,
    address: Option<super::addressing::NodeAddress>,
    capabilities: Vec<String>,
    last_seen: Option<u64>,
}

impl NodeInfoBuilder {
    /// Sets the node ID.
    #[must_use]
    pub fn id(mut self, id: NodeId) -> Self {
        self.id = Some(id);
        self
    }

    /// Sets the node address.
    #[must_use]
    pub fn address(mut self, address: super::addressing::NodeAddress) -> Self {
        self.address = Some(address);
        self
    }

    /// Adds a capability.
    #[must_use]
    pub fn capability(mut self, capability: String) -> Self {
        self.capabilities.push(capability);
        self
    }

    /// Sets all capabilities.
    #[must_use]
    pub fn capabilities(mut self, capabilities: Vec<String>) -> Self {
        self.capabilities = capabilities;
        self
    }

    /// Sets the last seen timestamp.
    #[must_use]
    pub fn last_seen(mut self, timestamp: u64) -> Self {
        self.last_seen = Some(timestamp);
        self
    }

    /// Builds the `NodeInfo` instance.
    ///
    /// # Errors
    /// Returns `ValidationError` if required fields are missing.
    pub fn build(self) -> Result<NodeInfo, ValidationError> {
        let id = self.id.ok_or(ValidationError::EmptyId)?;
        let address = self.address.ok_or(ValidationError::EmptyAddress)?;
        let last_seen = self.last_seen.unwrap_or(0);

        NodeInfo::new(id, address, self.capabilities, last_seen)
    }
}

#[cfg(test)]
mod tests {
    use super::super::addressing::NodeAddress;
    use super::*;

    #[test]
    fn test_node_id_creation() {
        let id1 = NodeId::new();
        let id2 = NodeId::new();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_node_id_from_str() {
        let id = NodeId::from_str("test-node").unwrap();
        assert_eq!(id.as_str(), "test-node");
    }

    #[test]
    fn test_node_id_from_str_empty() {
        let result = NodeId::from_str("");
        assert!(matches!(result, Err(ValidationError::EmptyId)));
    }

    #[test]
    fn test_node_id_from_str_whitespace() {
        let result = NodeId::from_str("   ");
        assert!(matches!(result, Err(ValidationError::EmptyId)));
    }

    #[test]
    fn test_node_id_display() {
        let id = NodeId::from_str("test-node").unwrap();
        assert_eq!(id.to_string(), "test-node");
    }

    #[test]
    fn test_node_id_into_string() {
        let id = NodeId::from_str("test-node").unwrap();
        assert_eq!(id.into_string(), "test-node");
    }

    #[test]
    fn test_node_info_builder() {
        let info = NodeInfo::builder()
            .id(NodeId::from_str("node-1").unwrap())
            .address(NodeAddress::new("127.0.0.1:8080").unwrap())
            .capability("mesh".to_string())
            .last_seen(100)
            .build()
            .unwrap();

        assert_eq!(info.id().as_str(), "node-1");
        assert_eq!(info.address().as_str(), "127.0.0.1:8080");
        assert_eq!(info.capabilities(), &["mesh"]);
        assert_eq!(info.last_seen(), 100);
    }

    #[test]
    fn test_node_info_accessors() {
        let info = NodeInfo::new(
            NodeId::from_str("node-1").unwrap(),
            NodeAddress::new("127.0.0.1:8080").unwrap(),
            vec!["mesh".to_string()],
            100,
        )
        .unwrap();

        assert_eq!(info.id().as_str(), "node-1");
        assert_eq!(info.address().as_str(), "127.0.0.1:8080");
        assert_eq!(info.capabilities(), &["mesh"]);
        assert_eq!(info.last_seen(), 100);
    }

    #[test]
    fn test_node_info_update_last_seen() {
        let mut info = NodeInfo::new(
            NodeId::from_str("node-1").unwrap(),
            NodeAddress::new("127.0.0.1:8080").unwrap(),
            vec![],
            100,
        )
        .unwrap();

        info.update_last_seen(200);
        assert_eq!(info.last_seen(), 200);
    }

    #[test]
    fn test_node_info_add_capability() {
        let mut info = NodeInfo::new(
            NodeId::from_str("node-1").unwrap(),
            NodeAddress::new("127.0.0.1:8080").unwrap(),
            vec![],
            100,
        )
        .unwrap();

        info.add_capability("compute".to_string());
        assert_eq!(info.capabilities(), &["compute"]);
    }

    #[test]
    fn test_node_info_builder_missing_id() {
        let result = NodeInfo::builder()
            .address(NodeAddress::new("127.0.0.1:8080").unwrap())
            .build();
        assert!(matches!(result, Err(ValidationError::EmptyId)));
    }

    #[test]
    fn test_node_info_builder_missing_address() {
        let result = NodeInfo::builder()
            .id(NodeId::from_str("node-1").unwrap())
            .build();
        assert!(matches!(result, Err(ValidationError::EmptyAddress)));
    }

    #[test]
    fn test_serialization() {
        let info = NodeInfo::new(
            NodeId::from_str("node-1").unwrap(),
            NodeAddress::new("127.0.0.1:8080").unwrap(),
            vec!["mesh".to_string()],
            100,
        )
        .unwrap();

        let json = serde_json::to_string(&info).unwrap();
        let deserialized: NodeInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id(), info.id());
        assert_eq!(deserialized.address(), info.address());
    }
}
