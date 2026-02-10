//! `ReAct` loop implementation.

use crate::config::AgentConfig;
use crate::engine::state::AgentState;
use crate::error::{AgentError, ResultExt};
use crate::tools::ToolRegistry;
use crate::types::{ExecutionResult, InferenceResponse, Message, Role, TaskContext};
use std::time::{Duration, Instant};

/// Function type for performing inference.
pub type InferenceFn = dyn Fn(&str, &[Message]) -> Result<InferenceResponse, AgentError>;

/// Core engine that orchestrates agent execution.
pub struct AgentEngine {
    pub(crate) state: AgentState,
    pub(crate) tools: ToolRegistry,
    pub(crate) config: AgentConfig,
    pub(crate) start_time: Instant,
}

impl AgentEngine {
    /// Creates a new agent engine.
    ///
    /// # Errors
    ///
    /// Returns an error if the task description is empty.
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
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The timeout is exceeded
    /// - The maximum number of iterations is exceeded
    /// - Tool execution fails
    /// - Inference fails
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
            let response = self.model_response(inference_fn)?;

            // Execute tools from response
            let execution_result = self
                .tools
                .execute_all(&response.content)
                .map_err(AgentError::ToolExecution)?;

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
    ///
    /// # Errors
    ///
    /// Returns the same errors as [`run`](Self::run).
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
    fn model_response(&self, inference_fn: &InferenceFn) -> Result<InferenceResponse, AgentError> {
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
    #[must_use]
    pub fn iteration(&self) -> u32 {
        self.state.iteration
    }

    /// Returns the elapsed time since engine start.
    #[must_use]
    pub fn elapsed(&self) -> Duration {
        self.start_time.elapsed()
    }

    /// Returns a reference to the conversation history.
    #[must_use]
    pub fn history(&self) -> &[Message] {
        &self.state.history
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::AgentEngineBuilder;
    use crate::error::ToolError;
    use crate::tools::constants::control;
    use crate::tools::{Tool, ToolParser};
    use crate::types::TaskContext;
    use regex::Captures;
    use std::borrow::Cow;
    use std::collections::HashMap;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    fn create_test_context() -> TaskContext {
        TaskContext::new("test-123", "Test task")
    }

    fn create_test_registry() -> ToolRegistry {
        ToolRegistry::new()
    }

    fn create_mock_inference(
        responses: Vec<InferenceResponse>,
    ) -> impl Fn(&str, &[Message]) -> Result<InferenceResponse, AgentError> {
        let counter = AtomicUsize::new(0);
        let responses = Arc::new(responses);
        move |_model, _history| {
            let idx = counter.fetch_add(1, Ordering::SeqCst);
            match responses.get(idx) {
                Some(resp) => Ok(InferenceResponse {
                    content: resp.content.clone(),
                    model: resp.model.clone(),
                    tokens_used: resp.tokens_used,
                    finish_reason: resp.finish_reason.clone(),
                }),
                None => Ok(InferenceResponse {
                    content: "<done>Done</done>".to_string(),
                    model: "test".to_string(),
                    tokens_used: None,
                    finish_reason: None,
                }),
            }
        }
    }

    struct MockTool {
        name: Cow<'static, str>,
        description: Cow<'static, str>,
        should_fail: bool,
    }

    impl MockTool {
        fn new(name: impl Into<String>) -> Self {
            let name_str: String = name.into();
            let desc = format!("<{name_str} />");
            Self {
                name: Cow::Owned(name_str),
                description: Cow::Owned(desc),
                should_fail: false,
            }
        }

        fn failing(name: impl Into<String>) -> Self {
            let name_str: String = name.into();
            let desc = format!("<{name_str} />");
            Self {
                name: Cow::Owned(name_str),
                description: Cow::Owned(desc),
                should_fail: true,
            }
        }
    }

    impl Tool for MockTool {
        fn name(&self) -> Cow<'static, str> {
            self.name.clone()
        }

        fn description(&self) -> Cow<'static, str> {
            self.description.clone()
        }

        fn execute(&self, _args: &HashMap<String, String>) -> Result<String, ToolError> {
            if self.should_fail {
                Err(ToolError::ExecutionFailed {
                    tool: self.name.to_string(),
                    source: Box::new(std::io::Error::other(
                        "Mock tool failure",
                    )),
                })
            } else {
                Ok(format!("{} executed successfully", self.name))
            }
        }
    }

    fn create_done_parser() -> Arc<ToolParser> {
        Arc::new(
            ToolParser::new(r"<done>(.*?)</done>", |caps: &Captures| {
                let mut args = HashMap::new();
                if let Some(m) = caps.get(1) {
                    args.insert("summary".to_string(), m.as_str().to_string());
                }
                args
            })
            .unwrap(),
        )
    }

    fn create_mock_parser(tool_name: &str) -> Arc<ToolParser> {
        let pattern = format!(r"<{tool_name}>(.*?)</{tool_name}>");
        let name = tool_name.to_string();
        Arc::new(
            ToolParser::new(&pattern, move |caps: &Captures| {
                let mut args = HashMap::new();
                if let Some(m) = caps.get(1) {
                    args.insert("arg".to_string(), m.as_str().to_string());
                }
                args.insert("tool_name".to_string(), name.clone());
                args
            })
            .unwrap(),
        )
    }

    fn create_registry_with_done_tool() -> ToolRegistry {
        let mut registry = ToolRegistry::new();
        let done_tool = Box::new(MockTool::new(control::DONE));
        registry.register(control::DONE, done_tool, create_done_parser());
        registry
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

    #[test]
    fn test_run_single_iteration_success() {
        let ctx = create_test_context();
        let registry = create_registry_with_done_tool();
        let config = AgentConfig::default();

        let mut engine = AgentEngine::new(&ctx, registry, config).unwrap();

        let mock_inference = create_mock_inference(vec![InferenceResponse {
            content: "<done>Task completed</done>".to_string(),
            model: "test".to_string(),
            tokens_used: Some(100),
            finish_reason: Some("stop".to_string()),
        }]);

        let result = engine.run(&mock_inference);

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "Task completed");
        assert_eq!(engine.iteration(), 1);
    }

    #[test]
    fn test_run_multiple_iterations() {
        let ctx = create_test_context();
        let mut registry = ToolRegistry::new();

        let mock_tool = Box::new(MockTool::new("mock_tool"));
        registry.register("mock_tool", mock_tool, create_mock_parser("mock_tool"));

        let done_tool = Box::new(MockTool::new(control::DONE));
        registry.register(control::DONE, done_tool, create_done_parser());

        let config = AgentConfig::default();
        let mut engine = AgentEngine::new(&ctx, registry, config).unwrap();

        let mock_inference = create_mock_inference(vec![
            InferenceResponse {
                content: "<mock_tool>test</mock_tool>".to_string(),
                model: "test".to_string(),
                tokens_used: Some(50),
                finish_reason: None,
            },
            InferenceResponse {
                content: "<done>All done</done>".to_string(),
                model: "test".to_string(),
                tokens_used: Some(100),
                finish_reason: Some("stop".to_string()),
            },
        ]);

        let result = engine.run(&mock_inference);

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "All done");
        assert_eq!(engine.iteration(), 2);

        let history = engine.history();
        assert!(history.len() >= 3); // Initial user message + assistant + tool messages
    }

    #[test]
    fn test_run_timeout() {
        let ctx = create_test_context();
        let registry = create_registry_with_done_tool();
        let config = AgentConfig::builder()
            .timeout(Duration::from_millis(10))
            .build()
            .unwrap();

        let mut engine = AgentEngine::new(&ctx, registry, config).unwrap();

        let mock_inference = |_model: &str, _history: &[Message]| {
            std::thread::sleep(Duration::from_millis(50));
            Ok(InferenceResponse {
                content: "Still working...".to_string(),
                model: "test".to_string(),
                tokens_used: None,
                finish_reason: None,
            })
        };

        let result = engine.run(&mock_inference);

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), AgentError::Timeout { .. }));
    }

    #[test]
    fn test_run_max_iterations_exceeded() {
        let ctx = create_test_context();
        let registry = create_test_registry();
        let config = AgentConfig::builder().max_iterations(2).build().unwrap();

        let mut engine = AgentEngine::new(&ctx, registry, config).unwrap();

        let mock_inference = |_model: &str, _history: &[Message]| {
            Ok(InferenceResponse {
                content: "Let me think...".to_string(),
                model: "test".to_string(),
                tokens_used: None,
                finish_reason: None,
            })
        };

        let result = engine.run(&mock_inference);

        assert!(result.is_err());
        match result.unwrap_err() {
            AgentError::MaxIterationsExceeded { max } => {
                assert_eq!(max, 2);
            }
            _ => panic!("Expected MaxIterationsExceeded error"),
        }
    }

    #[test]
    fn test_run_tool_execution_failure() {
        let ctx = create_test_context();
        let mut registry = ToolRegistry::new();

        let failing_tool = Box::new(MockTool::failing("failing_tool"));
        registry.register(
            "failing_tool",
            failing_tool,
            create_mock_parser("failing_tool"),
        );

        let config = AgentConfig::default();
        let mut engine = AgentEngine::new(&ctx, registry, config).unwrap();

        let mock_inference = create_mock_inference(vec![InferenceResponse {
            content: "<failing_tool>trigger</failing_tool>".to_string(),
            model: "test".to_string(),
            tokens_used: None,
            finish_reason: None,
        }]);

        let result = engine.run(&mock_inference);

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            AgentError::ToolExecution { .. }
        ));
    }

    #[test]
    fn test_state_update_with_results() {
        let ctx = create_test_context();
        let mut registry = ToolRegistry::new();

        let mock_tool = Box::new(MockTool::new("mock_tool"));
        registry.register("mock_tool", mock_tool, create_mock_parser("mock_tool"));

        let done_tool = Box::new(MockTool::new(control::DONE));
        registry.register(control::DONE, done_tool, create_done_parser());

        let config = AgentConfig::default();
        let mut engine = AgentEngine::new(&ctx, registry, config).unwrap();

        let initial_history_len = engine.history().len();

        let mock_inference = create_mock_inference(vec![
            InferenceResponse {
                content: "<mock_tool>execute</mock_tool>".to_string(),
                model: "test".to_string(),
                tokens_used: None,
                finish_reason: None,
            },
            InferenceResponse {
                content: "<done>Complete</done>".to_string(),
                model: "test".to_string(),
                tokens_used: None,
                finish_reason: Some("stop".to_string()),
            },
        ]);

        let result = engine.run(&mock_inference);

        assert!(result.is_ok());

        let history = engine.history();
        assert!(history.len() > initial_history_len);

        let has_assistant = history.iter().any(|m| m.role == Role::Assistant);
        let has_tool = history.iter().any(|m| m.role == Role::Tool);

        assert!(has_assistant, "History should contain assistant messages");
        assert!(has_tool, "History should contain tool messages");
    }
}
