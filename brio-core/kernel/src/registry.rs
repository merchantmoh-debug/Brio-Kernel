//! Plugin registry for managing WebAssembly components.
//!
//! This module provides a registry for loading, managing, and instantiating
//! WASM plugins with proper permission scoping and host state injection.

use crate::engine::linker::create_linker;
use crate::host::BrioHostState;
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::fs;
use tracing::{info, warn};
use wasmtime::component::Component;
use wasmtime::{Engine, Store};

/// Metadata about a loaded plugin.
#[derive(Debug, Clone)]
pub struct PluginMetadata {
    /// Unique identifier for the plugin.
    pub id: String,
    /// Path to the plugin file.
    pub path: PathBuf,
    /// Permissions granted to the plugin.
    pub permissions: Vec<String>,
}

/// Registry for managing dynamic plugins.
pub struct PluginRegistry {
    plugins: HashMap<String, PluginMetadata>,
    engine: Engine,
}

impl PluginRegistry {
    /// Creates a new, empty registry.
    #[must_use]
    pub fn new(engine: Engine) -> Self {
        Self {
            plugins: HashMap::new(),
            engine,
        }
    }

    /// Returns a reference to the underlying WASM engine.
    #[must_use]
    pub fn engine(&self) -> &Engine {
        &self.engine
    }

    /// Scans a directory for .wasm files and registers them.
    ///
    /// # Errors
    ///
    /// Returns an error if the directory cannot be read.
    pub async fn load_from_directory<P: AsRef<Path>>(&mut self, path: P) -> Result<()> {
        let path = path.as_ref();
        if !path.exists() {
            warn!("Plugin directory does not exist: {:?}", path);
            return Ok(());
        }

        let mut entries = fs::read_dir(path).await?;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("wasm") {
                self.register_plugin(&path);
            }
        }
        Ok(())
    }

    /// Registers a single plugin file.
    fn register_plugin(&mut self, path: &Path) {
        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();

        info!("Loading plugin: {} from {:?}", name, path);

        let metadata = PluginMetadata {
            id: name.clone(),
            path: path.to_path_buf(),
            permissions: vec![],
        };

        self.plugins.insert(name, metadata);
    }

    /// Instantiates a plugin by ID.
    ///
    /// # Errors
    ///
    /// Returns an error if the plugin is not found, if the component fails to load,
    /// or if instantiation fails.
    pub async fn instantiate(
        &self,
        plugin_id: &str,
        host_state: BrioHostState,
    ) -> Result<Store<BrioHostState>> {
        let metadata = self
            .plugins
            .get(plugin_id)
            .ok_or_else(|| anyhow::anyhow!("Plugin not found: {plugin_id}"))?;

        let component = Component::from_file(&self.engine, &metadata.path)
            .context("Failed to load component")?;

        let linker = create_linker(&self.engine)?;

        // Create a view of host state with plugin context
        let plugin_state =
            host_state.with_plugin_context(plugin_id.to_string(), metadata.permissions.clone());

        let mut store = Store::new(&self.engine, plugin_state);

        let _ = linker.instantiate_async(&mut store, &component).await?;

        Ok(store)
    }

    /// Lists all registered plugins.
    ///
    /// # Returns
    ///
    /// A vector of plugin metadata.
    #[must_use]
    pub fn list_plugins(&self) -> Vec<PluginMetadata> {
        self.plugins.values().cloned().collect()
    }

    /// Gets metadata for a specific plugin.
    ///
    /// # Arguments
    ///
    /// * `id` - The ID of the plugin to look up.
    ///
    /// # Returns
    ///
    /// Plugin metadata if found.
    #[must_use]
    pub fn get(&self, id: &str) -> Option<PluginMetadata> {
        self.plugins.get(id).cloned()
    }
}
