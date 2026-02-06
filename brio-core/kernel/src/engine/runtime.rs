use crate::host::BrioHostState;
use anyhow::{Context, Result};
use wasmtime::component::{Component, Linker};
use wasmtime::{Engine, Store};

pub struct WasmEngine {
    engine: Engine,
    linker: Linker<BrioHostState>,
}

impl WasmEngine {
    /// Creates a new WASM engine with the given linker.
    ///
    /// # Errors
    ///
    /// Returns an error if the engine cannot be created from the linker.
    pub fn new(linker: Linker<BrioHostState>) -> Result<Self> {
        let engine = linker.engine().clone();
        Ok(Self { engine, linker })
    }

    /// Loads a component from the given path.
    ///
    /// # Errors
    ///
    /// Returns an error if the component cannot be loaded from the file.
    pub fn load_component(&self, path: &std::path::Path) -> Result<Component> {
        Component::from_file(&self.engine, path)
            .with_context(|| format!("Failed to load component from {}", path.display()))
    }

    #[must_use]
    pub fn prepare_store(&self, state: BrioHostState) -> Store<BrioHostState> {
        Store::new(&self.engine, state)
    }

    #[must_use]
    pub fn linker(&self) -> &Linker<BrioHostState> {
        &self.linker
    }
}
