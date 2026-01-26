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
    pub id: String,
    pub path: PathBuf,
    pub permissions: Vec<String>,
}

/// Registry for managing dynamic plugins.
pub struct PluginRegistry {
    plugins: HashMap<String, PluginMetadata>,
    engine: Engine,
}

impl PluginRegistry {
    /// Creates a new, empty registry.
    pub fn new(engine: Engine) -> Self {
        Self {
            plugins: HashMap::new(),
            engine,
        }
    }

    pub fn engine(&self) -> &Engine {
        &self.engine
    }

    /// Scans a directory for .wasm files and registers them.
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
                self.register_plugin(&path).await?;
            }
        }
        Ok(())
    }

    /// Registers a single plugin file.
    async fn register_plugin(&mut self, path: &Path) -> Result<()> {
        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();

        info!("Loading plugin: {} from {:?}", name, path);

        // TODO: inspecting the component to verify imports/exports or custom sections.
        // For now, we assume valid components.

        let metadata = PluginMetadata {
            id: name.clone(),
            path: path.to_path_buf(),
            permissions: vec![], // TODO: Load from custom section or config
        };

        self.plugins.insert(name, metadata);
        Ok(())
    }

    /// Instantiates a plugin by ID.
    pub async fn instantiate(
        &self,
        plugin_id: &str,
        host_state: BrioHostState,
    ) -> Result<Store<BrioHostState>> {
        let metadata = self
            .plugins
            .get(plugin_id)
            .ok_or_else(|| anyhow::anyhow!("Plugin not found: {}", plugin_id))?;

        let component = Component::from_file(&self.engine, &metadata.path)
            .context("Failed to load component from file")?;

        let linker = create_linker(&self.engine)?;

        // Apply plugin permissions to the host state
        let host_state = host_state.with_permissions(metadata.permissions.clone());

        let mut store = Store::new(&self.engine, host_state);

        let _ = linker.instantiate_async(&mut store, &component).await?;

        Ok(store)
    }

    pub fn list_plugins(&self) -> Vec<PluginMetadata> {
        self.plugins.values().cloned().collect()
    }

    pub fn get(&self, id: &str) -> Option<PluginMetadata> {
        self.plugins.get(id).cloned()
    }
}
