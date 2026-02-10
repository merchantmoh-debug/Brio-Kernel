//! Core types for mesh networking.

pub mod addressing;
pub mod node;
pub mod registry;

pub use addressing::NodeAddress;
pub use node::{NodeId, NodeInfo, NodeInfoBuilder, ValidationError};
pub use registry::{MeshConfig, MeshConfigBuilder};
