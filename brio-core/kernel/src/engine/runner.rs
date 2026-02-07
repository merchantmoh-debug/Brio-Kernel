//! Agent runner for executing WASM components.
//!
//! This module provides the runtime execution environment for smart agents,
//! handling task execution and event processing through WASM component instantiation.

use crate::host::BrioHostState;
use anyhow::{Context, Result};
use wasmtime::component::Component;
use wasmtime::{Engine, Store};

wasmtime::component::bindgen!({
    inline: r#"
        package brio:core;

        interface agent-runner {
            record task-context {
                task-id: string,
                description: string,
                input-files: list<string>,
            }
        
            run: func(context: task-context) -> result<string, string>;
        }

        interface event-handler {
            variant payload {
                json(string),
                binary(list<u8>)
            }
            handle-event: func(topic: string, data: payload);
        }

        world smart-agent {
            import agent-runner;
            export agent-runner; 
            export event-handler; 
        }
    "#,
    world: "smart-agent",
    additional_derives: [serde::Deserialize, serde::Serialize],
});

pub type SmartAgentInstance = SmartAgent;
pub use exports::brio::core::agent_runner::TaskContext;
pub use exports::brio::core::event_handler::Payload as EventPayload;

pub struct AgentRunner {
    engine: Engine,
}

impl AgentRunner {
    #[must_use]
    pub fn new(engine: Engine) -> Self {
        Self { engine }
    }

    /// Instantiates an agent component and runs it.
    ///
    /// # Errors
    ///
    /// Returns an error if the component fails to load, instantiate, or execute.
    pub async fn run_agent(
        &self,
        component_path: &std::path::Path,
        host_state: BrioHostState,
        context: exports::brio::core::agent_runner::TaskContext,
    ) -> Result<String> {
        let component = Component::from_file(&self.engine, component_path)
            .context("Failed to load component")?;

        let linker = crate::engine::linker::create_linker(&self.engine)?;

        let mut store = Store::new(&self.engine, host_state);

        let agent = SmartAgent::instantiate_async(&mut store, &component, &linker).await?;

        let result = agent
            .brio_core_agent_runner()
            .call_run(&mut store, &context)?;

        result.map_err(|e| anyhow::anyhow!("Agent execution failed: {e}"))
    }

    /// Runs an event handler for a given component.
    ///
    /// # Errors
    ///
    /// Returns an error if the component fails to load, instantiate, or handle the event.
    pub async fn run_event_handler(
        &self,
        component_path: &std::path::Path,
        host_state: BrioHostState,
        topic: String,
        payload: exports::brio::core::event_handler::Payload,
    ) -> Result<()> {
        let component = Component::from_file(&self.engine, component_path)
            .context("Failed to load component")?;

        let linker = crate::engine::linker::create_linker(&self.engine)?;
        let mut store = Store::new(&self.engine, host_state);

        let agent = SmartAgent::instantiate_async(&mut store, &component, &linker).await?;

        agent
            .brio_core_event_handler()
            .call_handle_event(&mut store, &topic, &payload)?;

        Ok(())
    }
}
