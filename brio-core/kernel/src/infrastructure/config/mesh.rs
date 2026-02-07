//! Mesh networking configuration for the Brio kernel.
//!
//! This module defines distributed mesh node settings.

use serde::Deserialize;

/// Mesh networking settings.
#[derive(Debug, Deserialize, Clone)]
pub struct MeshSettings {
    /// Unique identifier for this node.
    pub node_id: Option<String>,
    /// Port to listen on for mesh connections.
    pub port: Option<u16>,
}
