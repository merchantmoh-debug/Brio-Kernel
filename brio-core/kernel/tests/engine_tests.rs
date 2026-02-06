//! Tests for the WASM engine module.

use anyhow::Result;
use brio_kernel::engine::{WasmEngine, create_engine_config, create_linker};
use brio_kernel::host::BrioHostState;
use brio_kernel::inference::{ChatRequest, ChatResponse, InferenceError, LLMProvider};

// =============================================================================
// Mock Provider
// =============================================================================

struct MockProvider;

#[async_trait::async_trait]
impl LLMProvider for MockProvider {
    async fn chat(&self, _request: ChatRequest) -> Result<ChatResponse, InferenceError> {
        Err(InferenceError::ProviderError("Mock".to_string()))
    }
}

// =============================================================================
// Engine Configuration Tests
// =============================================================================

#[test]
fn engine_config_should_create_valid_engine_with_component_model() {
    let config = create_engine_config();
    let engine = wasmtime::Engine::new(&config);
    assert!(engine.is_ok());
}

// =============================================================================
// Linker Tests
// =============================================================================

#[test]
fn create_linker_should_succeed_with_valid_engine() {
    let config = create_engine_config();
    let engine = wasmtime::Engine::new(&config).unwrap();
    let linker = create_linker(&engine);
    assert!(linker.is_ok());
}

// =============================================================================
// WasmEngine Tests
// =============================================================================

#[test]
fn wasm_engine_should_create_successfully_with_valid_linker() {
    let config = create_engine_config();
    let engine = wasmtime::Engine::new(&config).unwrap();
    let linker = create_linker(&engine).unwrap();
    let wasm_engine = WasmEngine::new(linker);
    assert!(wasm_engine.is_ok());
}

#[tokio::test]
async fn wasm_engine_should_prepare_store_with_host_state() -> Result<()> {
    let config = create_engine_config();
    let engine = wasmtime::Engine::new(&config)?;
    let linker = create_linker(&engine)?;
    let wasm_engine = WasmEngine::new(linker)?;

    let host_state =
        BrioHostState::with_provider("sqlite::memory:", Box::new(MockProvider)).await?;
    let _store = wasm_engine.prepare_store(host_state);

    Ok(())
}

#[test]
fn wasm_engine_should_provide_linker_access() {
    let config = create_engine_config();
    let engine = wasmtime::Engine::new(&config).unwrap();
    let linker = create_linker(&engine).unwrap();
    let wasm_engine = WasmEngine::new(linker).unwrap();

    let _linker_ref = wasm_engine.linker();
}

#[test]
fn load_component_should_return_error_for_invalid_path() {
    let config = create_engine_config();
    let engine = wasmtime::Engine::new(&config).unwrap();
    let linker = create_linker(&engine).unwrap();
    let wasm_engine = WasmEngine::new(linker).unwrap();

    // Loading a non-existent path should fail
    let result = wasm_engine.load_component(std::path::Path::new("/nonexistent/component.wasm"));
    assert!(result.is_err());
}

// =============================================================================
// Empty Component Tests
// =============================================================================

#[tokio::test]
async fn instantiate_should_succeed_for_empty_component() -> Result<()> {
    let config = create_engine_config();
    let engine = wasmtime::Engine::new(&config)?;
    let linker = create_linker(&engine)?;
    let wasm_engine = WasmEngine::new(linker)?;

    // Create minimal empty component
    let component = wasmtime::component::Component::new(&engine, r"(component)")?;

    let host_state =
        BrioHostState::with_provider("sqlite::memory:", Box::new(MockProvider)).await?;
    let mut store = wasm_engine.prepare_store(host_state);

    // Should be able to instantiate
    let instance = wasm_engine
        .linker()
        .instantiate_async(&mut store, &component)
        .await;
    assert!(instance.is_ok());

    Ok(())
}

// =============================================================================
// Engine Clone Test
// =============================================================================

#[tokio::test]
async fn engine_should_support_multiple_stores_concurrently() {
    let config = create_engine_config();
    let engine = wasmtime::Engine::new(&config).unwrap();
    let linker = create_linker(&engine).unwrap();
    let wasm_engine = WasmEngine::new(linker).unwrap();

    // Create multiple stores from the same engine
    let host1 = BrioHostState::with_provider("sqlite::memory:", Box::new(MockProvider))
        .await
        .unwrap();
    let host2 = BrioHostState::with_provider("sqlite::memory:", Box::new(MockProvider))
        .await
        .unwrap();

    let _store1 = wasm_engine.prepare_store(host1);
    let _store2 = wasm_engine.prepare_store(host2);
}
