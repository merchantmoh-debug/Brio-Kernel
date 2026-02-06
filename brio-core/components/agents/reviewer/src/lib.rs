//! Reviewer Agent - A specialized agent for code review and analysis.
//!
//! This agent uses the agent-sdk to provide code review capabilities.
//! Note: This agent intentionally does NOT have write permissions for safety.

use agent_sdk::{
    AgentConfig, AgentEngineBuilder, InferenceResponse, Message, PromptBuilder, Role, TaskContext,
    ToolRegistry,
};
use anyhow::Result;
use std::collections::HashMap;
use wit_bindgen::generate;

// Generate WIT bindings at crate root level
generate!({
    world: "smart-agent",
    path: "../../../wit",
    skip: ["tool"],
    generate_all,
});

/// ReviewerAgent implements the agent-runner and event-handler interfaces.
pub struct ReviewerAgent;

impl exports::brio::core::agent_runner::Guest for ReviewerAgent {
    fn run(context: exports::brio::core::agent_runner::TaskContext) -> Result<String, String> {
        // Convert WIT context to SDK TaskContext
        let task_context = convert_wit_context(&context);

        // Load configuration
        let config_raw =
            AgentConfig::from_env().map_err(|e| format!("Configuration error: {}", e))?;
        let config = config_raw
            .validate()
            .map_err(|e| format!("Validation error: {}", e))?;

        // Create tool registry with reviewer-specific tools (no write tool for safety)
        let tools = create_tool_registry(&config);

        // Build the agent engine
        let mut engine = AgentEngineBuilder::new()
            .tools(tools)
            .config(config.clone())
            .build(&task_context)
            .map_err(|e| format!("Failed to initialize agent: {}", e))?;

        // Build system prompt
        let system_prompt = PromptBuilder::build_reviewer_agent(
            &task_context,
            &create_tool_registry(&config),
            &config,
        );

        // Create inference function
        let inference_fn = |model: &str,
                            history: &[Message]|
         -> Result<InferenceResponse, agent_sdk::AgentError> {
            Self::perform_inference(model, history)
        };

        // Run the agent
        engine
            .run_with_prompt(system_prompt, &inference_fn)
            .map_err(|e| format!("Agent execution failed: {}", e))
    }
}

impl exports::brio::core::event_handler::Guest for ReviewerAgent {
    fn handle_event(topic: String, data: exports::brio::core::event_handler::Payload) {
        let data_str = match &data {
            exports::brio::core::event_handler::Payload::Json(s) => s.clone(),
            exports::brio::core::event_handler::Payload::Binary(_) => "[Binary data]".to_string(),
        };

        brio::core::logging::log(
            brio::core::logging::Level::Info,
            "reviewer-agent",
            &format!("Received event on topic '{}': {}", topic, data_str),
        );
    }
}

impl ReviewerAgent {
    /// Performs inference using the AI interface.
    fn perform_inference(
        model: &str,
        history: &[Message],
    ) -> Result<InferenceResponse, agent_sdk::AgentError> {
        let wit_messages: Vec<brio::ai::inference::Message> = history
            .iter()
            .map(|msg| brio::ai::inference::Message {
                role: convert_role(&msg.role),
                content: msg.content.clone(),
            })
            .collect();

        let response = brio::ai::inference::chat(model, &wit_messages).map_err(|e| {
            agent_sdk::AgentError::Inference(agent_sdk::InferenceError::ApiError(format!(
                "{:?}",
                e
            )))
        })?;

        let tokens_used = response.usage.as_ref().map(|u| u.total_tokens);

        Ok(InferenceResponse {
            content: response.content,
            model: model.to_string(),
            tokens_used,
            finish_reason: None,
        })
    }
}

/// Converts SDK Role to WIT Role.
fn convert_role(role: &Role) -> brio::ai::inference::Role {
    match role {
        Role::System => brio::ai::inference::Role::System,
        Role::User => brio::ai::inference::Role::User,
        Role::Assistant | Role::Tool => brio::ai::inference::Role::Assistant,
    }
}

/// Converts WIT TaskContext to SDK TaskContext.
fn convert_wit_context(context: &exports::brio::core::agent_runner::TaskContext) -> TaskContext {
    TaskContext::new(&context.task_id, &context.description).with_files(context.input_files.clone())
}

/// Creates a tool registry with reviewer-specific tools.
/// NOTE: This agent intentionally does NOT have write tool for safety.
fn create_tool_registry(config: &AgentConfig) -> ToolRegistry {
    let mut registry = ToolRegistry::new();

    // Always register done tool
    registry.register("done", Box::new(DoneTool), create_done_parser());

    // Register read tool
    registry.register(
        "read_file",
        Box::new(ReadFileTool::new(config.max_file_size)),
        create_read_parser(),
    );

    // Register list tool
    registry.register(
        "ls",
        Box::new(ListDirectoryTool::new(config.max_depth)),
        create_list_parser(),
    );

    // Reviewer does NOT have write tool for safety

    registry
}

fn create_done_parser() -> agent_sdk::tools::ToolParser {
    use std::collections::HashMap;
    agent_sdk::tools::ToolParser::new(r#"<done>\s*(.*?)\s*</done>"#, |caps: &regex::Captures| {
        let mut args = HashMap::new();
        args.insert("summary".to_string(), caps[1].to_string());
        args
    })
    .expect("Invalid regex pattern")
}

fn create_read_parser() -> agent_sdk::tools::ToolParser {
    use std::collections::HashMap;
    agent_sdk::tools::ToolParser::new(
        r#"<read_file\s+path="([^"]+)"\s*/?>"#,
        |caps: &regex::Captures| {
            let mut args = HashMap::new();
            args.insert("path".to_string(), caps[1].to_string());
            args
        },
    )
    .expect("Invalid regex pattern")
}

fn create_list_parser() -> agent_sdk::tools::ToolParser {
    use std::collections::HashMap;
    agent_sdk::tools::ToolParser::new(r#"<ls\s+path="([^"]+)"\s*/?>"#, |caps: &regex::Captures| {
        let mut args = HashMap::new();
        args.insert("path".to_string(), caps[1].to_string());
        args
    })
    .expect("Invalid regex pattern")
}

// Tool implementations that wrap the SDK tools
use agent_sdk::Tool;
use agent_sdk::error::ToolError;

struct ReadFileTool {
    max_size: u64,
}

impl ReadFileTool {
    fn new(max_size: u64) -> Self {
        Self { max_size }
    }
}

impl Tool for ReadFileTool {
    fn name(&self) -> &str {
        "read_file"
    }

    fn description(&self) -> &str {
        r#"<read_file path="path/to/file" /> - Read content from a file"#
    }

    fn execute(&self, args: HashMap<String, String>) -> Result<String, ToolError> {
        let path = args
            .get("path")
            .ok_or_else(|| ToolError::InvalidArguments {
                tool: "read_file".to_string(),
                reason: "Missing 'path' argument".to_string(),
            })?;

        // Check file size first
        let metadata = std::fs::metadata(path).map_err(|e| ToolError::ExecutionFailed {
            tool: "read_file".to_string(),
            source: Box::new(e),
        })?;

        if metadata.len() > self.max_size {
            return Err(ToolError::ResourceLimitExceeded {
                tool: "read_file".to_string(),
                resource: format!(
                    "file size ({} bytes, max: {})",
                    metadata.len(),
                    self.max_size
                ),
            });
        }

        std::fs::read_to_string(path).map_err(|e| ToolError::ExecutionFailed {
            tool: "read_file".to_string(),
            source: Box::new(e),
        })
    }
}

struct ListDirectoryTool {
    #[allow(dead_code)]
    max_depth: usize,
}

impl ListDirectoryTool {
    fn new(max_depth: usize) -> Self {
        Self { max_depth }
    }
}

impl Tool for ListDirectoryTool {
    fn name(&self) -> &str {
        "ls"
    }

    fn description(&self) -> &str {
        r#"<ls path="path/to/directory" /> - List directory contents"#
    }

    fn execute(&self, args: HashMap<String, String>) -> Result<String, ToolError> {
        let path = args
            .get("path")
            .ok_or_else(|| ToolError::InvalidArguments {
                tool: "ls".to_string(),
                reason: "Missing 'path' argument".to_string(),
            })?;

        let entries: Vec<String> = std::fs::read_dir(path)
            .map_err(|e| ToolError::ExecutionFailed {
                tool: "ls".to_string(),
                source: Box::new(e),
            })?
            .filter_map(|entry| entry.ok())
            .map(|entry| {
                let name = entry.file_name().to_string_lossy().to_string();
                let ty = if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                    "📁"
                } else {
                    "📄"
                };
                format!("{} {}", ty, name)
            })
            .collect();

        Ok(entries.join("\n"))
    }
}

struct DoneTool;

impl Tool for DoneTool {
    fn name(&self) -> &str {
        "done"
    }

    fn description(&self) -> &str {
        r#"<done>summary of completion</done> - Mark task as complete"#
    }

    fn execute(&self, _args: HashMap<String, String>) -> Result<String, ToolError> {
        Ok("Task marked as complete".to_string())
    }
}

export!(ReviewerAgent);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_role_conversion() {
        assert!(matches!(
            convert_role(&Role::System),
            brio::ai::inference::Role::System
        ));
        assert!(matches!(
            convert_role(&Role::User),
            brio::ai::inference::Role::User
        ));
        assert!(matches!(
            convert_role(&Role::Assistant),
            brio::ai::inference::Role::Assistant
        ));
        assert!(matches!(
            convert_role(&Role::Tool),
            brio::ai::inference::Role::Assistant
        ));
    }

    #[test]
    fn test_context_conversion() {
        let wit_ctx = exports::brio::core::agent_runner::TaskContext {
            task_id: "test-123".to_string(),
            description: "Test task".to_string(),
            input_files: vec!["file1.rs".to_string(), "file2.rs".to_string()],
        };

        let ctx = convert_wit_context(&wit_ctx);

        assert_eq!(ctx.task_id, "test-123");
        assert_eq!(ctx.description, "Test task");
        assert_eq!(ctx.file_count(), 2);
        assert!(ctx.has_input_files());
    }
}
