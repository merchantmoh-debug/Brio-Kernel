//! Host state management for the Brio kernel.
//!
//! This module provides the core `BrioHostState` struct which serves as the
//! central coordination point for all kernel operations.

use anyhow::{Context, Result, anyhow};
use parking_lot::{Mutex, RwLock};
use sqlx::{SqlitePool, sqlite::SqlitePoolOptions};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc::Sender;

use crate::branch_manager::BranchManager;
use crate::inference::{LLMProvider, ProviderRegistry};
use crate::infrastructure::config::SandboxSettings;
use crate::mesh::MeshMessage;
use crate::mesh::events::EventBus;
use crate::mesh::remote::RemoteRouter;
use crate::mesh::types::{NodeId, NodeInfo};
use crate::registry::PluginRegistry;
use crate::store::{PrefixPolicy, SqlStore};
use crate::vfs::manager::SessionManager;
use crate::ws::Broadcaster;

use super::permissions::PermissionChecker;

/// Inner state that can be cheaply cloned via Arc.
pub(crate) struct BrioHostStateInner {
    pub(crate) mesh_router: Arc<RwLock<HashMap<String, Sender<MeshMessage>>>>,
    pub(crate) remote_router: Option<RemoteRouter>,
    pub(crate) db_pool: SqlitePool,
    pub(crate) broadcaster: Broadcaster,
    pub(crate) session_manager: Arc<Mutex<SessionManager>>,
    pub(crate) provider_registry: Arc<ProviderRegistry>,
    pub(crate) permissions: Arc<std::collections::HashSet<String>>,
    pub(crate) plugin_registry: Option<Arc<PluginRegistry>>,
    pub(crate) event_bus: Arc<EventBus>,
    pub(crate) current_plugin_id: Option<String>,
    pub(crate) branch_manager: Arc<BranchManager>,
}

/// The main host state for the Brio kernel.
///
/// This struct serves as the central coordination point for all kernel operations,
/// managing sessions, inference providers, mesh networking, and plugin execution.
/// It uses an internal `Arc` for cheap cloning and thread-safe sharing.
#[derive(Clone)]
pub struct BrioHostState {
    pub(crate) inner: Arc<BrioHostStateInner>,
}

impl std::fmt::Debug for BrioHostState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BrioHostState")
            .field("current_plugin_id", &self.inner.current_plugin_id)
            .finish_non_exhaustive()
    }
}

impl BrioHostState {
    /// Creates a new `BrioHostState` with a pre-configured provider registry.
    ///
    /// # Errors
    ///
    /// Returns an error if the database connection fails or if the session manager
    /// cannot be initialized.
    pub async fn new(
        db_url: &str,
        registry: ProviderRegistry,
        plugin_registry: Option<Arc<PluginRegistry>>,
        sandbox: SandboxSettings,
    ) -> Result<Self> {
        let pool = SqlitePoolOptions::new().connect(db_url).await?;

        Ok(Self {
            inner: Arc::new(BrioHostStateInner {
                mesh_router: Arc::new(RwLock::new(HashMap::new())),
                remote_router: None, // Default to standalone mode
                db_pool: pool,
                broadcaster: Broadcaster::new(),
                session_manager: Arc::new(Mutex::new(
                    SessionManager::new(&sandbox)
                        .context("Failed to initialize session manager")?,
                )),
                provider_registry: Arc::new(registry),
                permissions: Arc::new(std::collections::HashSet::new()),
                plugin_registry,
                event_bus: Arc::new(EventBus::new()),
                current_plugin_id: None,
                branch_manager: Arc::new(BranchManager::new()),
            }),
        })
    }

    /// Creates a new `BrioHostState` with distributed mesh support.
    ///
    /// # Errors
    ///
    /// Returns an error if the database connection fails or if the session manager
    /// cannot be initialized.
    pub async fn new_distributed(
        db_url: &str,
        registry: ProviderRegistry,
        plugin_registry: Option<Arc<PluginRegistry>>,
        _node_id: NodeId,
        sandbox: SandboxSettings,
    ) -> Result<Self> {
        let pool = SqlitePoolOptions::new().connect(db_url).await?;
        let remote_router = RemoteRouter::new();

        Ok(Self {
            inner: Arc::new(BrioHostStateInner {
                mesh_router: Arc::new(RwLock::new(HashMap::new())),
                remote_router: Some(remote_router),
                db_pool: pool,
                broadcaster: Broadcaster::new(),
                session_manager: Arc::new(Mutex::new(
                    SessionManager::new(&sandbox)
                        .context("Failed to initialize session manager in distributed mode")?,
                )),
                provider_registry: Arc::new(registry),
                permissions: Arc::new(std::collections::HashSet::new()),
                plugin_registry,
                event_bus: Arc::new(EventBus::new()),
                current_plugin_id: None,
                branch_manager: Arc::new(BranchManager::new()),
            }),
        })
    }

    /// Creates a new `BrioHostState` with a single provider (backward compatible).
    ///
    /// # Errors
    ///
    /// Returns an error if the database connection fails or if the session manager
    /// cannot be initialized.
    pub async fn with_provider(db_url: &str, provider: Box<dyn LLMProvider>) -> Result<Self> {
        let registry = ProviderRegistry::new();
        registry.register_arc("default", Arc::from(provider));
        registry.set_default("default");
        Self::new(db_url, registry, None, SandboxSettings::default()).await
    }

    /// Registers a component with the mesh router.
    pub fn register_component(&self, id: impl Into<String>, sender: Sender<MeshMessage>) {
        let mut router = self.inner.mesh_router.write();
        router.insert(id.into(), sender);
    }

    /// Registers a remote node with the mesh router.
    ///
    /// This allows the host to route messages to components running on remote nodes.
    pub fn register_remote_node(&self, info: NodeInfo) {
        if let Some(router) = &self.inner.remote_router {
            router.register_node(info);
        }
    }

    /// Returns a reference to the database connection pool.
    #[must_use]
    pub fn db(&self) -> &SqlitePool {
        &self.inner.db_pool
    }

    /// Creates a new SQL store instance for the given scope.
    ///
    /// The store provides key-value storage with prefix-based namespacing.
    #[must_use]
    pub fn store(&self, _scope: &str) -> SqlStore {
        SqlStore::new(self.inner.db_pool.clone(), Box::new(PrefixPolicy))
    }

    /// Returns a reference to the WebSocket broadcaster for real-time updates.
    #[must_use]
    pub fn broadcaster(&self) -> &Broadcaster {
        &self.inner.broadcaster
    }

    /// Broadcasts a WebSocket patch to all connected clients.
    ///
    /// # Errors
    ///
    /// Returns an error if the broadcast channel is closed or full.
    pub fn broadcast_patch(&self, patch: crate::ws::WsPatch) -> Result<()> {
        self.broadcaster()
            .broadcast(crate::ws::BroadcastMessage::Patch(Box::new(patch)))
            .map_err(|e| anyhow!("Broadcast failed: {e}"))
    }

    /// Begins a new VFS session for the given base path.
    ///
    /// # Errors
    ///
    /// Returns an error if the session cannot be created (see [`SessionManager::begin_session`]).
    pub fn begin_session(&self, base_path: &str) -> Result<String, crate::vfs::SessionError> {
        let mut manager = self.inner.session_manager.lock();
        manager.begin_session(base_path)
    }

    /// Commits changes from a VFS session back to the base directory.
    ///
    /// # Errors
    ///
    /// Returns an error if the session cannot be committed (see [`SessionManager::commit_session`]).
    pub fn commit_session(&self, session_id: &str) -> Result<(), crate::vfs::SessionError> {
        let mut manager = self.inner.session_manager.lock();
        manager.commit_session(session_id)
    }

    /// Rolls back a session, discarding all changes.
    ///
    /// # Arguments
    /// * `session_id` - The session identifier returned by `begin_session`
    ///
    /// # Errors
    /// Returns an error if the session doesn't exist or cleanup fails.
    pub fn rollback_session(&self, session_id: &str) -> Result<(), crate::vfs::SessionError> {
        let mut manager = self.inner.session_manager.lock();
        manager.rollback_session(session_id)
    }

    /// Returns the provider registry for multi-model access.
    #[must_use]
    pub fn registry(&self) -> Arc<ProviderRegistry> {
        self.inner.provider_registry.clone()
    }

    /// Returns a specific LLM provider by name.
    ///
    /// # Arguments
    ///
    /// * `name` - The registered name of the provider to retrieve.
    ///
    /// # Returns
    ///
    /// An `Arc` to the provider if found, or `None` if no provider with that name exists.
    #[must_use]
    pub fn inference_by_name(&self, name: &str) -> Option<Arc<dyn LLMProvider>> {
        self.inner.provider_registry.get(name)
    }

    /// Returns the default LLM provider.
    ///
    /// This is a backward-compatible convenience method that returns the default
    /// configured provider, or `None` if no default is set.
    #[must_use]
    pub fn inference(&self) -> Option<Arc<dyn LLMProvider>> {
        self.inner.provider_registry.default_provider()
    }

    /// Creates a new view of the host state with restricted permissions and plugin context.
    #[must_use]
    pub fn with_plugin_context(&self, plugin_id: String, permissions: Vec<String>) -> Self {
        let inner = BrioHostStateInner {
            mesh_router: Arc::clone(&self.inner.mesh_router),
            remote_router: self.inner.remote_router.clone(),
            db_pool: self.inner.db_pool.clone(),
            broadcaster: self.inner.broadcaster.clone(),
            session_manager: Arc::clone(&self.inner.session_manager),
            provider_registry: Arc::clone(&self.inner.provider_registry),
            permissions: Arc::new(permissions.into_iter().collect()),
            plugin_registry: self.inner.plugin_registry.clone(),
            event_bus: Arc::clone(&self.inner.event_bus),
            current_plugin_id: Some(plugin_id),
            branch_manager: Arc::clone(&self.inner.branch_manager),
        };
        Self {
            inner: Arc::new(inner),
        }
    }

    /// Returns a reference to the event bus for mesh communication.
    #[must_use]
    pub fn event_bus(&self) -> &EventBus {
        &self.inner.event_bus
    }

    /// Returns the ID of the currently executing plugin, if any.
    #[must_use]
    pub fn current_plugin_id(&self) -> Option<&str> {
        self.inner.current_plugin_id.as_deref()
    }

    /// Returns the plugin registry if available.
    #[must_use]
    pub fn plugin_registry(&self) -> Option<Arc<PluginRegistry>> {
        self.inner.plugin_registry.clone()
    }

    /// Returns the branch manager for branch operations.
    #[must_use]
    pub fn branch_manager(&self) -> Arc<BranchManager> {
        self.inner.branch_manager.clone()
    }

    /// Get a reference to the mesh router (internal use).
    #[allow(dead_code)]
    pub(crate) fn mesh_router(&self) -> &Arc<RwLock<HashMap<String, Sender<MeshMessage>>>> {
        &self.inner.mesh_router
    }

    /// Get a reference to the remote router (internal use).
    #[allow(dead_code)]
    pub(crate) fn remote_router(&self) -> Option<&RemoteRouter> {
        self.inner.remote_router.as_ref()
    }

    /// Get a reference to the plugin registry (internal use).
    #[allow(dead_code)]
    pub(crate) fn plugin_registry_ref(&self) -> Option<&Arc<PluginRegistry>> {
        self.inner.plugin_registry.as_ref()
    }

    /// Get a reference to the session manager (internal use).
    pub(crate) fn session_manager(&self) -> &Arc<Mutex<SessionManager>> {
        &self.inner.session_manager
    }
}

impl PermissionChecker for BrioHostState {
    fn check_permission(&self, permission: &str) -> Result<(), String> {
        if self.inner.permissions.contains(permission) {
            Ok(())
        } else {
            Err(super::permissions::PermissionError::PermissionDenied {
                permission: permission.to_string(),
            }
            .to_string())
        }
    }
}
