use crate::host::BrioHostState;
use anyhow::{Context, Result};
use wasmtime::component::{Component, Linker};
use wasmtime::{Engine, Store};

pub struct WasmEngine {
    engine: Engine,
    linker: Linker<BrioHostState>,
}

impl WasmEngine {
    pub fn new(linker: Linker<BrioHostState>) -> Result<Self> {
        let engine = linker.engine().clone();
        Ok(Self { engine, linker })
    }

    pub fn load_component(&self, path: &std::path::Path) -> Result<Component> {
        Component::from_file(&self.engine, path)
            .with_context(|| format!("Failed to load component from {:?}", path))
    }

    pub fn prepare_store(&self, state: BrioHostState) -> Store<BrioHostState> {
        Store::new(&self.engine, state)
    }

    pub fn linker(&self) -> &Linker<BrioHostState> {
        &self.linker
    }
}
