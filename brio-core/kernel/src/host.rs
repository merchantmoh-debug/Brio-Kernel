use anyhow::{Result, anyhow};
use sqlx::{SqlitePool, sqlite::SqlitePoolOptions};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc::Sender;
use tokio::sync::oneshot;

use crate::inference::{LLMProvider, ProviderRegistry};
use crate::mesh::remote::RemoteRouter;
use crate::mesh::types::{NodeId, NodeInfo};
use crate::mesh::{MeshMessage, Payload};
use crate::registry::PluginRegistry;
use crate::store::{PrefixPolicy, SqlStore};
use crate::vfs::manager::SessionManager;
use crate::ws::{BroadcastMessage, Broadcaster, WsPatch};

#[derive(Clone)]
pub struct BrioHostState {
    mesh_router: Arc<std::sync::RwLock<HashMap<String, Sender<MeshMessage>>>>,
    remote_router: Option<RemoteRouter>,
    db_pool: SqlitePool,
    broadcaster: Broadcaster,
    session_manager: Arc<std::sync::Mutex<SessionManager>>,
    provider_registry: Arc<ProviderRegistry>,
    permissions: Arc<std::collections::HashSet<String>>,
    plugin_registry: Option<Arc<PluginRegistry>>,
}

impl BrioHostState {
    /// Creates a new BrioHostState with a pre-configured provider registry.
    pub async fn new(
        db_url: &str,
        registry: ProviderRegistry,
        plugin_registry: Option<Arc<PluginRegistry>>,
        sandbox: crate::infrastructure::config::SandboxSettings,
    ) -> Result<Self> {
        let pool = SqlitePoolOptions::new().connect(db_url).await?;

        Ok(Self {
            mesh_router: Arc::new(std::sync::RwLock::new(HashMap::new())),
            remote_router: None, // Default to standalone mode
            db_pool: pool,
            broadcaster: Broadcaster::new(),
            session_manager: Arc::new(std::sync::Mutex::new(SessionManager::new(sandbox))),
            provider_registry: Arc::new(registry),
            permissions: Arc::new(std::collections::HashSet::new()),
            plugin_registry,
        })
    }

    /// Creates a new BrioHostState with distributed mesh support
    pub async fn new_distributed(
        db_url: &str,
        registry: ProviderRegistry,
        plugin_registry: Option<Arc<PluginRegistry>>,
        _node_id: NodeId,
        sandbox: crate::infrastructure::config::SandboxSettings,
    ) -> Result<Self> {
        let pool = SqlitePoolOptions::new().connect(db_url).await?;
        let remote_router = RemoteRouter::new();

        Ok(Self {
            mesh_router: Arc::new(std::sync::RwLock::new(HashMap::new())),
            remote_router: Some(remote_router),
            db_pool: pool,
            broadcaster: Broadcaster::new(),
            session_manager: Arc::new(std::sync::Mutex::new(SessionManager::new(sandbox))),
            provider_registry: Arc::new(registry),
            permissions: Arc::new(std::collections::HashSet::new()),
            plugin_registry,
        })
    }

    /// Creates a new BrioHostState with a single provider (backward compatible).
    pub async fn with_provider(db_url: &str, provider: Box<dyn LLMProvider>) -> Result<Self> {
        let registry = ProviderRegistry::new();
        registry.register_arc("default", Arc::from(provider));
        registry.set_default("default");
        Self::new(db_url, registry, None, Default::default()).await
    }

    pub fn register_component(&self, id: String, sender: Sender<MeshMessage>) {
        let mut router = self.mesh_router.write().expect("RwLock poisoned");
        router.insert(id, sender);
    }

    pub fn register_remote_node(&self, info: NodeInfo) {
        if let Some(router) = &self.remote_router {
            router.register_node(info);
        }
    }

    pub fn db(&self) -> &SqlitePool {
        &self.db_pool
    }

    pub fn get_store(&self, _scope: &str) -> SqlStore {
        SqlStore::new(self.db_pool.clone(), Box::new(PrefixPolicy))
    }

    pub fn broadcaster(&self) -> &Broadcaster {
        &self.broadcaster
    }

    pub fn broadcast_patch(&self, patch: WsPatch) -> Result<()> {
        self.broadcaster
            .broadcast(BroadcastMessage::Patch(patch))
            .map_err(|e| anyhow!("Broadcast failed: {}", e))
    }

    pub async fn mesh_call(&self, target: &str, method: &str, payload: Payload) -> Result<Payload> {
        // 1. Try local routing first
        let sender = {
            let router = self.mesh_router.read().expect("RwLock poisoned");
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
                .map_err(|e| anyhow!("Failed to send message to target '{}': {}", target, e))?;
            let response = reply_rx
                .await
                .map_err(|e| anyhow!("Failed to receive reply from target '{}': {}", target, e))?;
            return response.map_err(|e| anyhow!("Target '{}' returned error: {}", target, e));
        }

        // 2. Try remote routing if enabled and target is formatted as "node_id/component"
        // Explicit remote addressing: "node_id/component_id"
        if let (Some(router), Some((node_id_str, component))) =
            (&self.remote_router, target.split_once('/'))
        {
            let node_id = NodeId::from(node_id_str.to_string());

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
        if let Some(registry) = &self.plugin_registry {
            if let Some(metadata) = registry.get(target) {
                use crate::engine::runner::{AgentRunner, TaskContext};

                let context: TaskContext = match payload {
                    Payload::Json(s) => serde_json::from_str(&s)
                        .map_err(|e| anyhow!("Invalid task context: {}", e))?,
                    _ => return Err(anyhow!("Agents only support JSON payload")),
                };

                let runner = AgentRunner::new(registry.engine().clone());
                let result = runner
                    .run_agent(&metadata.path, self.clone(), context)
                    .await?;
                return Ok(Payload::Json(result));
            }
        }

        Err(anyhow!(
            "Target component '{}' not found. Ensure format is 'component' (local) or 'node_id/component' (remote).",
            target
        ))
    }

    pub fn begin_session(&self, base_path: String) -> Result<String, String> {
        let mut manager = self.session_manager.lock().expect("Mutex poisoned");
        manager.begin_session(base_path)
    }

    pub fn commit_session(&self, session_id: String) -> Result<(), String> {
        let mut manager = self.session_manager.lock().expect("Mutex poisoned");
        manager.commit_session(session_id)
    }

    /// Returns the provider registry for multi-model access.
    pub fn registry(&self) -> Arc<ProviderRegistry> {
        self.provider_registry.clone()
    }

    /// Returns a specific LLM provider by name.
    pub fn inference_by_name(&self, name: &str) -> Option<Arc<dyn LLMProvider>> {
        self.provider_registry.get(name)
    }

    /// Returns the default LLM provider (backward compatible).
    pub fn inference(&self) -> Option<Arc<dyn LLMProvider>> {
        self.provider_registry.get_default()
    }

    /// Creates a new view of the host state with restricted permissions.
    pub fn with_permissions(&self, permissions: Vec<String>) -> Self {
        let mut new_state = self.clone();
        new_state.permissions = Arc::new(permissions.into_iter().collect());
        new_state
    }

    /// Checks if a permission is granted.
    ///
    /// # Errors
    /// Returns error if permission is denied.
    pub fn check_permission(&self, permission: &str) -> Result<(), String> {
        if self.permissions.contains(permission) {
            Ok(())
        } else {
            Err(format!("Permission denied: required '{}'", permission))
        }
    }
}
