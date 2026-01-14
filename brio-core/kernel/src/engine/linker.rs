use crate::engine::brio;
use crate::host::BrioHostState;
use anyhow::Result;
use wasmtime::component::{HasSelf, Linker};
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

impl brio::core::logging::Host for BrioHostState {
    fn log(&mut self, level: brio::core::logging::Level, context: String, message: String) {
        tracing::info!(
            target: "wasm_guest",
            level = %LogLevel(level),
            context = context,
            "[WASM] {}",
            message
        );
    }
}

struct LogLevel(brio::core::logging::Level);

impl std::fmt::Display for LogLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.0 {
            brio::core::logging::Level::Trace => f.write_str("TRACE"),
            brio::core::logging::Level::Debug => f.write_str("DEBUG"),
            brio::core::logging::Level::Info => f.write_str("INFO"),
            brio::core::logging::Level::Warn => f.write_str("WARN"),
            brio::core::logging::Level::Error => f.write_str("ERROR"),
        }
    }
}

pub fn create_linker(engine: &Engine) -> Result<Linker<BrioHostState>> {
    let mut linker = Linker::new(engine);
    register_host_interfaces(&mut linker)?;
    Ok(linker)
}

pub fn create_engine_config() -> Config {
    let mut config = Config::new();
    config.wasm_component_model(true);
    config.async_support(true);
    config
}

fn register_host_interfaces(linker: &mut Linker<BrioHostState>) -> Result<()> {
    type State = HasSelf<BrioHostState>;

    brio::core::service_mesh::add_to_linker::<BrioHostState, State>(linker, |s| s)?;
    brio::core::sql_state::add_to_linker::<BrioHostState, State>(linker, |s| s)?;
    brio::core::session_fs::add_to_linker::<BrioHostState, State>(linker, |s| s)?;
    brio::core::inference::add_to_linker::<BrioHostState, State>(linker, |s| s)?;
    brio::core::logging::add_to_linker::<BrioHostState, State>(linker, |s| s)?;

    Ok(())
}

fn stub_error(interface: &str) -> String {
    format!("Interface '{}' not yet implemented via WASM", interface)
}
