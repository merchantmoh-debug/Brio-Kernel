use crate::engine::brio;
use crate::host::BrioHostState;
use crate::mesh::Payload;
use anyhow::Result;
use wasmtime::component::{HasSelf, Linker};
use wasmtime::{Config, Engine};

impl brio::core::service_mesh::Host for BrioHostState {
    fn call(
        &mut self,
        target: String,
        method: String,
        args: brio::core::service_mesh::Payload,
    ) -> Result<brio::core::service_mesh::Payload, String> {
        // Convert WASM payload to internal Payload
        let internal_payload = match args {
            brio::core::service_mesh::Payload::Json(s) => Payload::Json(s),
            brio::core::service_mesh::Payload::Binary(b) => Payload::Binary(b),
        };

        // Bridge sync to async
        let result = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current()
                .block_on(async { self.mesh_call(&target, &method, internal_payload).await })
        });

        // Convert result back to WASM payload
        result
            .map(|p| match p {
                Payload::Json(s) => brio::core::service_mesh::Payload::Json(s),
                Payload::Binary(b) => brio::core::service_mesh::Payload::Binary(b),
            })
            .map_err(|e| e.to_string())
    }
}

impl brio::core::sql_state::Host for BrioHostState {
    fn query(
        &mut self,
        sql: String,
        params: Vec<String>,
    ) -> Result<Vec<brio::core::sql_state::Row>, String> {
        // Use a default scope for WASM guests
        let scope = "wasm_guest";
        let store = self.get_store(scope);

        let result = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current()
                .block_on(async { store.query(scope, &sql, params).await })
        });

        result
            .map(|rows| {
                rows.into_iter()
                    .map(|r| brio::core::sql_state::Row {
                        columns: r.columns,
                        values: r.values,
                    })
                    .collect()
            })
            .map_err(|e| e.to_string())
    }

    fn execute(&mut self, sql: String, params: Vec<String>) -> Result<u32, String> {
        let scope = "wasm_guest";
        let store = self.get_store(scope);

        let result = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current()
                .block_on(async { store.execute(scope, &sql, params).await })
        });

        result.map_err(|e| e.to_string())
    }
}

impl brio::core::session_fs::Host for BrioHostState {
    fn begin_session(&mut self, base_path: String) -> Result<String, String> {
        BrioHostState::begin_session(self, base_path)
    }

    fn commit_session(&mut self, session_id: String) -> Result<(), String> {
        BrioHostState::commit_session(self, session_id)
    }
}

impl brio::core::inference::Host for BrioHostState {
    fn chat(
        &mut self,
        model: String,
        messages: Vec<brio::core::inference::Message>,
    ) -> Result<brio::core::inference::CompletionResponse, brio::core::inference::InferenceError>
    {
        use crate::inference::{ChatRequest, Message, Role};

        // Convert WASM messages to internal messages
        let internal_messages: Vec<Message> = messages
            .into_iter()
            .map(|m| Message {
                role: match m.role {
                    brio::core::inference::Role::System => Role::System,
                    brio::core::inference::Role::User => Role::User,
                    brio::core::inference::Role::Assistant => Role::Assistant,
                },
                content: m.content,
            })
            .collect();

        let request = ChatRequest {
            model,
            messages: internal_messages,
        };

        let inference_provider = match self.inference() {
            Some(provider) => provider,
            None => {
                return Err(brio::core::inference::InferenceError::ProviderError(
                    "No default inference provider configured".to_string(),
                ));
            }
        };
        let result = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current()
                .block_on(async { inference_provider.chat(request).await })
        });

        result
            .map(|response| brio::core::inference::CompletionResponse {
                content: response.content,
                usage: response.usage.map(|u| brio::core::inference::Usage {
                    prompt_tokens: u.prompt_tokens,
                    completion_tokens: u.completion_tokens,
                    total_tokens: u.total_tokens,
                }),
            })
            .map_err(|e| match e {
                crate::inference::InferenceError::RateLimit => {
                    brio::core::inference::InferenceError::RateLimit
                }
                crate::inference::InferenceError::ContextLengthExceeded => {
                    brio::core::inference::InferenceError::ContextLengthExceeded
                }
                other => brio::core::inference::InferenceError::ProviderError(other.to_string()),
            })
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
