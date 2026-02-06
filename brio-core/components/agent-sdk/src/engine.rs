//! Agent engine with ReAct loop and state management.

use crate::config::AgentConfig;
use crate::error::{AgentError, ResultExt};
use crate::tools::ToolRegistry;
use crate::types::{ExecutionResult, InferenceResponse, Message, Role, TaskContext};
use std::time::{Duration, Instant};

/// Function type for performing inference.
pub type InferenceFn = dyn Fn(&str, &[Message]) -> Result<InferenceResponse, AgentError>;

/// Core engine that orchestrates agent execution.
pub struct AgentEngine {
    state: AgentState,
    tools: ToolRegistry,
    config: AgentConfig,
    start_time: Instant,
}

/// Internal state for the agent.
struct AgentState {
    history: Vec<Message>,
    iteration: u32,
}

impl AgentEngine {
    /// Creates a new agent engine.
    pub fn new(
        context: &TaskContext,
        tools: ToolRegistry,
        config: AgentConfig,
    ) -> Result<Self, AgentError> {
        // Validate task description
        if context.description.trim().is_empty() {
            return Err(AgentError::Task(
                crate::error::TaskError::InvalidDescription(
                    "Task description cannot be empty".to_string(),
                ),
            ));
        }

        let state = AgentState::new(Vec::new());

        Ok(Self {
            state,
            tools,
            config,
            start_time: Instant::now(),
        })
    }

    /// Runs the agent until completion or max iterations.
    pub fn run(&mut self, inference_fn: &InferenceFn) -> Result<String, AgentError> {
        // Add initial user message
        self.state.add_message(
            Role::User,
            "Please analyze the task and begin execution. Think step-by-step and use appropriate tools."
                .to_string(),
        );

        loop {
            // Check timeout
            if self.start_time.elapsed() > self.config.timeout {
                return Err(AgentError::Timeout {
                    elapsed: self.start_time.elapsed(),
                });
            }

            // Check max iterations
            if self.state.iteration >= self.config.max_iterations {
                return Err(AgentError::MaxIterationsExceeded {
                    max: self.config.max_iterations,
                });
            }

            self.state.iteration += 1;

            if self.config.verbose {
                tracing::info!(
                    "Agent iteration {}/{}",
                    self.state.iteration,
                    self.config.max_iterations
                );
            }

            // Get model response
            let response = self.get_model_response(inference_fn)?;

            // Execute tools from response
            let execution_result = self
                .tools
                .execute_all(&response.content)
                .map_err(|e| AgentError::ToolExecution(e))?;

            // Check for completion
            if execution_result.is_complete {
                return Ok(execution_result
                    .summary
                    .unwrap_or_else(|| "Task completed successfully".to_string()));
            }

            // Update state with results
            self.update_state(&response.content, &execution_result);
        }
    }

    /// Runs the agent with a system prompt.
    pub fn run_with_prompt(
        &mut self,
        system_prompt: impl Into<String>,
        inference_fn: &InferenceFn,
    ) -> Result<String, AgentError> {
        // Insert system prompt at the beginning
        self.state.history.insert(0, Message::system(system_prompt));

        self.run(inference_fn)
    }

    /// Gets the model response via the inference function.
    fn get_model_response(
        &self,
        inference_fn: &InferenceFn,
    ) -> Result<InferenceResponse, AgentError> {
        inference_fn(&self.config.model, &self.state.history)
            .with_context(|| "Failed to get model response")
    }

    /// Updates the agent state with the response and tool results.
    fn update_state(&mut self, assistant_response: &str, execution_result: &ExecutionResult) {
        // Add assistant message
        self.state
            .add_message(Role::Assistant, assistant_response.to_string());

        // Add tool results or user feedback
        if execution_result.output.trim().is_empty() {
            self.state.add_message(
                Role::User,
                "No tools were called. Please use available tools to make progress, or mark as <done> if complete."
                    .to_string(),
            );
        } else {
            let feedback = if execution_result.output.contains("✗") {
                format!(
                    "Tool execution completed with errors:\n{}\n\nPlease review and try again.",
                    execution_result.output
                )
            } else {
                format!("Tool execution results:\n{}", execution_result.output)
            };
            self.state.add_message(Role::Tool, feedback);
        }
    }

    /// Returns the current iteration count.
    pub fn iteration(&self) -> u32 {
        self.state.iteration
    }

    /// Returns the elapsed time since engine start.
    pub fn elapsed(&self) -> Duration {
        self.start_time.elapsed()
    }

    /// Returns a reference to the conversation history.
    pub fn history(&self) -> &[Message] {
        &self.state.history
    }
}

impl AgentState {
    fn new(history: Vec<Message>) -> Self {
        Self {
            history,
            iteration: 0,
        }
    }

    fn add_message(&mut self, role: Role, content: String) {
        self.history.push(Message::new(role, content));
    }
}

/// Builder for constructing agent engines.
#[derive(Debug)]
pub struct AgentEngineBuilder {
    tools: Option<ToolRegistry>,
    config: Option<AgentConfig>,
}

impl AgentEngineBuilder {
    /// Creates a new builder.
    pub fn new() -> Self {
        Self {
            tools: None,
            config: None,
        }
    }

    /// Sets the tool registry.
    pub fn tools(mut self, tools: ToolRegistry) -> Self {
        self.tools = Some(tools);
        self
    }

    /// Sets the configuration.
    pub fn config(mut self, config: AgentConfig) -> Self {
        self.config = Some(config);
        self
    }

    /// Builds the agent engine.
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::TaskContext;

    fn create_test_context() -> TaskContext {
        TaskContext::new("test-123", "Test task")
    }

    fn create_test_registry() -> ToolRegistry {
        ToolRegistry::new()
    }

    #[test]
    fn test_engine_creation_success() {
        let ctx = create_test_context();
        let registry = create_test_registry();
        let config = AgentConfig::default();

        let engine = AgentEngine::new(&ctx, registry, config);
        assert!(engine.is_ok());
    }

    #[test]
    fn test_engine_creation_empty_description() {
        let ctx = TaskContext::new("test", "   ");
        let registry = create_test_registry();
        let config = AgentConfig::default();

        let result = AgentEngine::new(&ctx, registry, config);
        assert!(result.is_err());
    }

    #[test]
    fn test_builder_pattern() {
        let ctx = create_test_context();
        let registry = create_test_registry();
        let config = AgentConfig::default();

        let result = AgentEngineBuilder::new()
            .tools(registry)
            .config(config)
            .build(&ctx);

        assert!(result.is_ok());
    }

    #[test]
    fn test_builder_missing_tools() {
        let ctx = create_test_context();

        let result = AgentEngineBuilder::new().build(&ctx);
        assert!(result.is_err());
    }
}
