//! Integration tests for the Brio kernel.
//!
//! These tests verify the core functionality of the WASM engine and host state.

#![allow(missing_docs)]

use anyhow::Result;
use brio_kernel::engine::{WasmEngine, create_engine_config, create_linker};
use brio_kernel::host::BrioHostState;
use brio_kernel::inference::{ChatRequest, ChatResponse, InferenceError, LLMProvider};

struct MockProvider;

#[async_trait::async_trait]
impl LLMProvider for MockProvider {
    async fn chat(&self, _request: ChatRequest) -> Result<ChatResponse, InferenceError> {
        Ok(ChatResponse {
            content: "Mock response".to_string(),
            usage: None,
        })
    }
}

#[tokio::test]
async fn host_should_initialize_and_instantiate_component() -> Result<()> {
    let host_state =
        BrioHostState::with_provider("sqlite::memory:", Box::new(MockProvider)).await?;

    let config = create_engine_config();
    let engine = wasmtime::Engine::new(&config)?;
    let linker = create_linker(&engine)?;
    let wasm_engine = WasmEngine::new(linker)?;

    let mut store = wasm_engine.prepare_store(host_state);

    let component = wasmtime::component::Component::new(&engine, r"(component)")?;
    let _instance = wasm_engine
        .linker()
        .instantiate_async(&mut store, &component)
        .await?;

    Ok(())
}
