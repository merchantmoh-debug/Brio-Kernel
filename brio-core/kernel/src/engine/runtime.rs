//! WASM runtime engine for executing WebAssembly components.
//!
//! This module provides a high-level wrapper around wasmtime for loading
//! and executing WASM components with the Brio host state.

use crate::host::BrioHostState;
use anyhow::{Context, Result};
use wasmtime::component::{Component, Linker};
use wasmtime::{Engine, Store};

/// High-level WASM engine for executing WebAssembly components.
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

    /// Prepares a new WASM store with the given host state.
    ///
    /// # Arguments
    ///
    /// * `state` - The host state to use in the store.
    ///
    /// # Returns
    ///
    /// A new `Store` initialized with the engine and host state.
    #[must_use]
    pub fn prepare_store(&self, state: BrioHostState) -> Store<BrioHostState> {
        Store::new(&self.engine, state)
    }

    /// Returns a reference to the linker.
    ///
    /// # Returns
    ///
    /// A reference to the `Linker` used by this engine.
    #[must_use]
    pub fn linker(&self) -> &Linker<BrioHostState> {
        &self.linker
    }
}
