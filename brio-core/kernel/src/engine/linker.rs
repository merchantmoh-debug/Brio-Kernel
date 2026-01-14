use crate::engine::brio;
use crate::host::BrioHostState;
use anyhow::Result;
use wasmtime::component::Linker;
use wasmtime::{Config, Engine};

impl brio::core::service_mesh::Host for BrioHostState {
    fn call(
        &mut self,
        _target: String,
        _method: String,
        _args: brio::core::service_mesh::Payload,
    ) -> Result<brio::core::service_mesh::Payload, String> {
        Err(stub_error("service-mesh"))
    }
}

impl brio::core::sql_state::Host for BrioHostState {
    fn query(
        &mut self,
        _sql: String,
        _params: Vec<String>,
    ) -> Result<Vec<brio::core::sql_state::Row>, String> {
        Err(stub_error("sql-state"))
    }

    fn execute(&mut self, _sql: String, _params: Vec<String>) -> Result<u32, String> {
        Err(stub_error("sql-state"))
    }
}

impl brio::core::session_fs::Host for BrioHostState {
    fn begin_session(&mut self, _base_path: String) -> Result<String, String> {
        Err(stub_error("session-fs"))
    }

    fn commit_session(&mut self, _session_id: String) -> Result<(), String> {
        Err(stub_error("session-fs"))
    }
}

impl brio::core::inference::Host for BrioHostState {
    fn chat(
        &mut self,
        _model: String,
        _messages: Vec<brio::core::inference::Message>,
    ) -> Result<brio::core::inference::CompletionResponse, brio::core::inference::InferenceError>
    {
        Err(brio::core::inference::InferenceError::ProviderError(
            stub_error("inference"),
        ))
    }
}

pub fn create_linker(engine: &Engine) -> Result<Linker<BrioHostState>> {
    let linker = Linker::new(engine);

    Ok(linker)
}

pub fn create_engine_config() -> Config {
    let mut config = Config::new();
    config.wasm_component_model(true);
    config.async_support(true);
    config
}

fn stub_error(interface: &str) -> String {
    format!("Interface '{}' not yet implemented via WASM", interface)
}
