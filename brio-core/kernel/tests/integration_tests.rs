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
async fn test_basic_host_interaction() -> Result<()> {
    // 1. Setup host state
    let host_state =
        BrioHostState::with_provider("sqlite::memory:", Box::new(MockProvider)).await?;

    // 2. Setup WASM engine
    let config = create_engine_config();
    let engine = wasmtime::Engine::new(&config)?;
    let linker = create_linker(&engine)?;
    let wasm_engine = WasmEngine::new(linker)?;

    // 3. Prepare store (this injects host state)
    let mut store = wasm_engine.prepare_store(host_state);

    // 4. Create a dummy component that does nothing just to verify instantiation
    let component = wasmtime::component::Component::new(&engine, r#"(component)"#)?;
    let _instance = wasm_engine
        .linker()
        .instantiate_async(&mut store, &component)
        .await?;

    // Verify instantiation succeeded
    // assert!(instance.exports(&mut store).root().into_iter().count() >= 0);

    Ok(())
}
