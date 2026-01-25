use anyhow::Result;
use brio_kernel::engine::{WasmEngine, create_engine_config, create_linker};
use brio_kernel::host::BrioHostState;
use brio_kernel::inference::{ChatRequest, ChatResponse, InferenceError, LLMProvider};

struct MockProvider;

#[async_trait::async_trait]
impl LLMProvider for MockProvider {
    async fn chat(&self, _req: ChatRequest) -> Result<ChatResponse, InferenceError> {
        Err(InferenceError::ProviderError("Mock".to_string()))
    }
}

#[tokio::test]
async fn wasm_engine_instantiates_empty_component() -> Result<()> {
    let host_state = create_host_state().await?;
    let engine = create_engine()?;
    let wasm_engine = create_wasm_engine(&engine)?;
    let component = load_empty_component(&engine)?;

    let mut store = wasm_engine.prepare_store(host_state);
    wasm_engine
        .linker()
        .instantiate_async(&mut store, &component)
        .await?;

    Ok(())
}

async fn create_host_state() -> Result<BrioHostState> {
    BrioHostState::with_provider("sqlite::memory:", Box::new(MockProvider)).await
}

fn create_engine() -> Result<wasmtime::Engine> {
    let config = create_engine_config();
    wasmtime::Engine::new(&config)
}

fn create_wasm_engine(engine: &wasmtime::Engine) -> Result<WasmEngine> {
    let linker = create_linker(engine)?;
    WasmEngine::new(linker)
}

fn load_empty_component(engine: &wasmtime::Engine) -> Result<wasmtime::component::Component> {
    wasmtime::component::Component::new(engine, r#"(component)"#)
}
