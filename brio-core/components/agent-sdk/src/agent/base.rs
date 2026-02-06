//! Base traits and functions for standard agents.
//!
//! This module provides the [`StandardAgent`] trait and [`run_standard_agent`] function
//! that implement the common `Guest::run` logic for standard AI-loop agents.

use crate::config::AgentConfig;
use crate::engine::AgentEngineBuilder;
use crate::error::{AgentError, InferenceError};
use crate::tools::ToolRegistry;
use crate::types::{InferenceResponse, Message, TaskContext};
use std::sync::Arc;
use thiserror::Error;

/// Configuration for a standard agent.
#[derive(Debug, Clone)]
pub struct StandardAgentConfig {
    /// Base agent configuration.
    pub agent_config: AgentConfig,
    /// Whether to enable event handling.
    pub enable_events: bool,
}

impl StandardAgentConfig {
    /// Creates a new standard agent configuration.
    #[must_use]
    pub fn new(agent_config: AgentConfig) -> Self {
        Self {
            agent_config,
            enable_events: true,
        }
    }

    /// Sets whether event handling is enabled.
    #[must_use]
    pub fn with_events(mut self, enabled: bool) -> Self {
        self.enable_events = enabled;
        self
    }
}

impl Default for StandardAgentConfig {
    fn default() -> Self {
        Self {
            agent_config: AgentConfig::default(),
            enable_events: true,
        }
    }
}

/// Errors specific to standard agent operations.
#[derive(Error, Debug)]
pub enum StandardAgentError {
    /// Configuration error.
    #[error("Configuration error: {0}")]
    Configuration(String),

    /// Agent execution error.
    #[error("Agent execution failed: {0}")]
    Execution(String),

    /// Inference error.
    #[error("Inference error: {0}")]
    Inference(#[from] InferenceError),

    /// Tool registry error.
    #[error("Tool registry error: {0}")]
    ToolRegistry(String),
}

/// Trait for standard AI-loop agents.
///
/// Implementors of this trait can use [`run_standard_agent`] to handle
/// the common `Guest::run` logic with standardized error handling and
/// event processing.
///
/// # Example
///
/// ```rust
/// use agent_sdk::agent::{StandardAgent, StandardAgentConfig};
/// use agent_sdk::types::{InferenceResponse, Message, TaskContext};
/// use agent_sdk::tools::ToolRegistry;
/// use agent_sdk::AgentError;
///
/// #[derive(Clone)]
/// struct MyAgent;
///
/// impl StandardAgent for MyAgent {
///     const NAME: &'static str = "my-agent";
///
///     fn build_prompt(
///         &self,
///         context: &TaskContext,
///         _tools: &ToolRegistry,
///         _config: &StandardAgentConfig,
///     ) -> String {
///         format!("You are {}. Task: {}", Self::NAME, context.description)
///     }
///
///     fn perform_inference(
///         &self,
///         _model: &str,
///         _history: &[Message],
///     ) -> Result<InferenceResponse, AgentError> {
///         Ok(InferenceResponse {
///             content: "Test".to_string(),
///             model: "test".to_string(),
///             tokens_used: None,
///             finish_reason: Some("stop".to_string()),
///         })
///     }
/// }
/// ```
pub trait StandardAgent: Clone + Send + Sync {
    /// The unique name of the agent.
    const NAME: &'static str;

    /// Builds the system prompt for this agent.
    ///
    /// # Arguments
    ///
    /// * `context` - The task context containing task metadata.
    /// * `tools` - The tool registry with available tools.
    /// * `config` - The standard agent configuration.
    ///
    /// # Returns
    ///
    /// The system prompt string to send to the AI model.
    fn build_prompt(
        &self,
        context: &TaskContext,
        tools: &ToolRegistry,
        config: &StandardAgentConfig,
    ) -> String;

    /// Creates the tool registry for this agent.
    ///
    /// # Arguments
    ///
    /// * `_config` - The agent configuration.
    ///
    /// # Returns
    ///
    /// A tool registry configured for this agent type.
    fn create_tool_registry(&self, _config: &AgentConfig) -> ToolRegistry {
        ToolRegistry::new()
    }

    /// Performs inference using the AI interface.
    ///
    /// # Arguments
    ///
    /// * `model` - The model identifier to use.
    /// * `history` - The conversation history.
    ///
    /// # Errors
    ///
    /// Returns an error if the inference fails.
    fn perform_inference(
        &self,
        model: &str,
        history: &[Message],
    ) -> Result<InferenceResponse, AgentError>;
}

/// Runs a standard agent with the given context and configuration.
///
/// This function handles the common logic for `Guest::run` implementations:
/// - Loads and validates configuration
/// - Creates the tool registry
/// - Builds the agent engine
/// - Runs the [`ReAct`] loop
///
/// # Type Parameters
///
/// * `A` - The agent type implementing [`StandardAgent`].
///
/// # Arguments
///
/// * `agent` - The agent instance to run.
/// * `context` - The task context.
/// * `config` - The standard agent configuration.
///
/// # Errors
///
/// Returns an error if:
/// - Configuration is invalid
/// - Tool registry creation fails
/// - Agent engine initialization fails
/// - Agent execution fails
///
/// # Example
///
/// ```rust,ignore
/// use agent_sdk::agent::{run_standard_agent, StandardAgent, StandardAgentConfig};
/// use agent_sdk::types::TaskContext;
///
/// let context = TaskContext::new("task-123", "Write a function");
/// let config = StandardAgentConfig::default();
/// let result = run_standard_agent(&MyAgent, &context, &config);
/// ```
pub fn run_standard_agent<A>(
    agent: &A,
    context: &TaskContext,
    config: &StandardAgentConfig,
) -> Result<String, StandardAgentError>
where
    A: StandardAgent + 'static,
{
    // Load and validate configuration
    let agent_config = AgentConfig::from_env()
        .map_err(|e| StandardAgentError::Configuration(format!("Failed to load config: {e}")))?;

    agent_config
        .validate()
        .map_err(|e| StandardAgentError::Configuration(format!("Invalid configuration: {e}")))?;

    // Create tool registry
    let tools = agent.create_tool_registry(&agent_config);

    // Build system prompt (must be done before moving tools)
    let system_prompt = agent.build_prompt(context, &tools, config);

    // Clone agent for the inference closure (moved into the closure)
    let agent_arc = Arc::new(agent.clone());

    // Build the agent engine
    let mut engine = AgentEngineBuilder::new()
        .tools(tools)
        .config(agent_config)
        .build(context)
        .map_err(|e| StandardAgentError::Execution(format!("Failed to initialize agent: {e}")))?;

    // Create inference function that captures the Arc
    let inference_fn =
        move |model: &str, history: &[Message]| -> Result<InferenceResponse, AgentError> {
            agent_arc.perform_inference(model, history)
        };

    // Run the agent
    engine
        .run_with_prompt(system_prompt, &inference_fn)
        .map_err(|e| StandardAgentError::Execution(format!("Agent execution failed: {e}")))
}

/// Handles a standard event for an agent.
///
/// This function provides a default event handler that logs events
/// at the appropriate level based on the topic.
///
/// # Arguments
///
/// * `agent_name` - The name of the agent handling the event.
/// * `topic` - The event topic.
/// * `data` - The event data as a string.
pub fn handle_standard_event(agent_name: &str, topic: &str, data: &str) {
    if topic.contains("error") {
        tracing::error!(
            agent = agent_name,
            topic = topic,
            data = data,
            "Received event"
        );
    } else if topic.contains("warn") {
        tracing::warn!(
            agent = agent_name,
            topic = topic,
            data = data,
            "Received event"
        );
    } else {
        tracing::info!(
            agent = agent_name,
            topic = topic,
            data = data,
            "Received event"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone)]
    struct TestAgent;

    impl StandardAgent for TestAgent {
        const NAME: &'static str = "test-agent";

        fn build_prompt(
            &self,
            context: &TaskContext,
            _tools: &ToolRegistry,
            _config: &StandardAgentConfig,
        ) -> String {
            format!("You are {}. Task: {}", Self::NAME, context.description)
        }

        fn perform_inference(
            &self,
            _model: &str,
            _history: &[Message],
        ) -> Result<InferenceResponse, AgentError> {
            Ok(InferenceResponse {
                content: "Test response".to_string(),
                model: "test-model".to_string(),
                tokens_used: None,
                finish_reason: Some("stop".to_string()),
            })
        }
    }

    #[test]
    fn test_standard_agent_config_default() {
        let config = StandardAgentConfig::default();
        assert!(config.enable_events);
        assert_eq!(config.agent_config.max_iterations, 20);
    }

    #[test]
    fn test_standard_agent_config_builder() {
        let agent_config = AgentConfig::default();
        let config = StandardAgentConfig::new(agent_config).with_events(false);
        assert!(!config.enable_events);
    }

    #[test]
    fn test_standard_agent_trait() {
        let agent = TestAgent;
        let context = TaskContext::new("test", "Do something");
        let config = StandardAgentConfig::default();
        let tools = ToolRegistry::new();

        let prompt = agent.build_prompt(&context, &tools, &config);
        assert!(prompt.contains("test-agent"));
        assert!(prompt.contains("Do something"));
    }

    #[test]
    fn test_handle_standard_event() {
        handle_standard_event("test-agent", "test-topic", "test-data");
        handle_standard_event("test-agent", "error-topic", "error-data");
        handle_standard_event("test-agent", "warn-topic", "warn-data");
    }
}
