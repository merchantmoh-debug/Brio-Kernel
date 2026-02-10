//! Mesh networking for the Brio kernel.
//!
//! This module provides mesh call handling functionality.

use anyhow::{Result, anyhow};
use std::sync::Arc;
use tokio::sync::oneshot;

use crate::engine::runner::{AgentRunner, TaskContext};
use crate::mesh::types::NodeId;
use crate::mesh::{MeshMessage, Payload};
use crate::registry::PluginRegistry;

use super::state::BrioHostState;

/// Trait for mesh handling functionality.
pub trait MeshHandler: Send + Sync {
    /// Calls a target component through the mesh network.
    ///
    /// Attempts local routing first, then remote routing, and finally on-demand plugin execution.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The target component is not found locally or remotely
    /// - The message send operation fails
    /// - Plugin execution fails (for agent targets)
    fn mesh_call(
        &self,
        target: &str,
        method: &str,
        payload: Payload,
    ) -> impl std::future::Future<Output = Result<Payload>> + Send;
}

impl MeshHandler for BrioHostState {
    async fn mesh_call(&self, target: &str, method: &str, payload: Payload) -> Result<Payload> {
        // 1. Try local routing first
        let sender = {
            let router = self.inner.mesh_router.read();
            router.get(target).cloned()
        };

        if let Some(sender) = sender {
            let (reply_tx, reply_rx) = oneshot::channel();
            let message = MeshMessage {
                target: target.to_string(),
                method: method.to_string(),
                payload,
                reply_tx,
            };

            sender
                .send(message)
                .await
                .map_err(|e| anyhow!("Failed to send message to target '{target}': {e}"))?;
            let response = reply_rx
                .await
                .map_err(|e| anyhow!("Failed to receive reply from target '{target}': {e}"))?;
            return response.map_err(|e| anyhow!("Target '{target}' returned error: {e}"));
        }

        // 2. Try remote routing if enabled and target is formatted as "node_id/component"
        // Explicit remote addressing: "node_id/component_id"
        if let (Some(router), Some((node_id_str, component))) =
            (&self.inner.remote_router, target.split_once('/'))
        {
            let node_id = NodeId::from_str(node_id_str).expect("valid node id");

            // If the target is a different node, route via gRPC
            let message = MeshMessage {
                target: component.to_string(),
                method: method.to_string(),
                payload,
                reply_tx: oneshot::channel().0, // Reply handling is managed by RemoteRouter's request/response flow
            };

            return router.send(&node_id, message).await;
        }

        // 3. Try on-demand plugin execution
        // We expect this lint because collapsing the nested if would require duplicating the
        // `registry` variable binding or restructuring the logic significantly, reducing readability.
        #[expect(clippy::collapsible_if)]
        if let Some(registry) = &self.inner.plugin_registry {
            if let Some(metadata) = registry.get(target) {
                let context: TaskContext = match payload {
                    Payload::Json(s) => serde_json::from_str(&s)
                        .map_err(|e| anyhow!("Invalid task context: {e}"))?,
                    Payload::Binary(_) => return Err(anyhow!("Agents only support JSON payload")),
                };

                let runner = AgentRunner::new(registry.engine().clone());
                let result = runner
                    .run_agent(&metadata.path, self.clone(), context)
                    .await?;
                return Ok(Payload::Json(Box::new(result)));
            }
        }

        Err(anyhow!(
            "Target component '{target}' not found. Ensure format is 'component' (local) or 'node_id/component' (remote)."
        ))
    }
}

/// Mesh routing information for a component.
#[derive(Clone)]
pub struct MeshRoute {
    /// Target component identifier.
    pub target: String,
    /// Whether the route is local or remote.
    pub route_type: RouteType,
}

impl std::fmt::Debug for MeshRoute {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MeshRoute")
            .field("target", &self.target)
            .field("route_type", &self.route_type)
            .finish()
    }
}

/// Type of mesh route.
#[derive(Clone)]
pub enum RouteType {
    /// Local route - component is running on this node.
    Local,
    /// Remote route - component is on another node.
    Remote {
        /// Node ID where the component is located.
        node_id: NodeId,
        /// Component ID on the remote node.
        component: String,
    },
    /// Plugin route - component is a plugin that can be loaded.
    Plugin {
        /// Plugin registry reference.
        registry: Arc<PluginRegistry>,
        /// Plugin metadata.
        metadata: crate::registry::PluginMetadata,
    },
}

impl std::fmt::Debug for RouteType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RouteType::Local => write!(f, "Local"),
            RouteType::Remote { node_id, component } => f
                .debug_struct("Remote")
                .field("node_id", node_id)
                .field("component", component)
                .finish(),
            RouteType::Plugin { .. } => write!(f, "Plugin"),
        }
    }
}

/// Parse a target string to determine the route type.
#[must_use]
pub fn parse_target(target: &str) -> Option<(&str, Option<&str>)> {
    target.split_once('/').map_or_else(
        || Some((target, None)),
        |(node_id, component)| Some((node_id, Some(component))),
    )
}
