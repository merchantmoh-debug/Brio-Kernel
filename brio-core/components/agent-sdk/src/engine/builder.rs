//! Engine builder pattern.

use crate::config::AgentConfig;
use crate::engine::AgentEngine;
use crate::error::AgentError;
use crate::tools::ToolRegistry;
use crate::types::TaskContext;

/// Builder for constructing agent engines.
#[derive(Debug)]
pub struct AgentEngineBuilder {
    tools: Option<ToolRegistry>,
    config: Option<AgentConfig>,
}

impl AgentEngineBuilder {
    /// Creates a new builder.
    #[must_use]
    pub fn new() -> Self {
        Self {
            tools: None,
            config: None,
        }
    }

    /// Sets the tool registry.
    #[must_use]
    pub fn tools(mut self, tools: ToolRegistry) -> Self {
        self.tools = Some(tools);
        self
    }

    /// Sets the configuration.
    #[must_use]
    pub fn config(mut self, config: AgentConfig) -> Self {
        self.config = Some(config);
        self
    }

    /// Builds the agent engine.
    ///
    /// # Errors
    ///
    /// Returns an error if the tool registry is not configured.
    pub fn build(self, context: &TaskContext) -> Result<AgentEngine, AgentError> {
        let tools = self.tools.ok_or_else(|| {
            AgentError::Task(crate::error::TaskError::MissingConfiguration {
                key: "tools".to_string(),
            })
        })?;

        let config = self.config.unwrap_or_default();

        AgentEngine::new(context, tools, config)
    }
}

impl Default for AgentEngineBuilder {
    fn default() -> Self {
        Self::new()
    }
}
