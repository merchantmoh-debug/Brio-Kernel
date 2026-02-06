//! Coder Agent - Code writing and modification
#![allow(missing_docs)]

use agent_sdk::agent::parsers::{
    create_done_parser, create_list_parser, create_read_parser, create_write_parser,
};
use agent_sdk::agent::tools::{DoneTool, ListDirectoryTool, ReadFileTool};
use agent_sdk::agent::{
    StandardAgent, StandardAgentConfig, handle_standard_event, run_standard_agent,
};
use agent_sdk::types::{InferenceResponse, TaskContext};
use agent_sdk::{
    AgentConfig, AgentError, InferenceError, Message, PromptBuilder, Role, Tool, ToolError,
    ToolRegistry,
};
use std::collections::HashMap;
use wit_bindgen::generate;

// Generate WIT bindings at crate root level
generate!({
    world: "smart-agent",
    path: "../../../wit",
    skip: ["tool"],
    generate_all,
});

/// `CoderAgent` implements the agent-runner and event-handler interfaces.
#[derive(Clone)]
pub struct CoderAgent;

impl StandardAgent for CoderAgent {
    const NAME: &'static str = "coder-agent";

    fn build_prompt(
        &self,
        context: &TaskContext,
        tools: &ToolRegistry,
        _config: &StandardAgentConfig,
    ) -> String {
        let config = AgentConfig::from_env().unwrap_or_default();
        PromptBuilder::build_coder_agent(context, tools, &config)
    }

    fn create_tool_registry(&self, config: &AgentConfig) -> ToolRegistry {
        let mut registry = ToolRegistry::new();

        // Register shared tools
        registry.register("done", Box::new(DoneTool), create_done_parser());
        registry.register(
            "read_file",
            Box::new(ReadFileTool::new(config.max_file_size)),
            create_read_parser(),
        );
        registry.register(
            "ls",
            Box::new(ListDirectoryTool::new(config.max_depth)),
            create_list_parser(),
        );

        // Coder has write capability (unlike reviewer)
        registry.register("write_file", Box::new(WriteFileTool), create_write_parser());

        registry
    }

    fn perform_inference(
        &self,
        model: &str,
        history: &[Message],
    ) -> Result<InferenceResponse, AgentError> {
        let wit_messages: Vec<brio::ai::inference::Message> = history
            .iter()
            .map(|msg| brio::ai::inference::Message {
                role: convert_role(msg.role),
                content: msg.content.clone(),
            })
            .collect();

        let response = brio::ai::inference::chat(model, &wit_messages)
            .map_err(|e| AgentError::Inference(InferenceError::ApiError(format!("{e:?}"))))?;

        let tokens_used = response.usage.as_ref().map(|u| u.total_tokens);

        Ok(InferenceResponse {
            content: response.content,
            model: model.to_string(),
            tokens_used,
            finish_reason: None,
        })
    }
}

impl exports::brio::core::agent_runner::Guest for CoderAgent {
    fn run(context: exports::brio::core::agent_runner::TaskContext) -> Result<String, String> {
        let task_context = TaskContext::new(&context.task_id, &context.description)
            .with_files(context.input_files);

        let config = StandardAgentConfig::default();

        run_standard_agent(&CoderAgent, &task_context, &config)
            .map_err(|e| format!("Agent execution failed: {e}"))
    }
}

impl exports::brio::core::event_handler::Guest for CoderAgent {
    fn handle_event(topic: String, data: exports::brio::core::event_handler::Payload) {
        let data_str = match &data {
            exports::brio::core::event_handler::Payload::Json(s) => s.clone(),
            exports::brio::core::event_handler::Payload::Binary(_) => "[Binary data]".to_string(),
        };

        handle_standard_event(Self::NAME, &topic, &data_str);
    }
}

/// Converts SDK `Role` to WIT `Role`.
fn convert_role(role: Role) -> brio::ai::inference::Role {
    match role {
        Role::System => brio::ai::inference::Role::System,
        Role::User => brio::ai::inference::Role::User,
        Role::Assistant | Role::Tool => brio::ai::inference::Role::Assistant,
    }
}

// WriteFileTool implementation (coder-specific version)
struct WriteFileTool;

impl Tool for WriteFileTool {
    fn name(&self) -> &'static str {
        "write_file"
    }

    fn description(&self) -> &'static str {
        r#"<write_file path="path/to/file">content</write_file> - Write content to a file"#
    }

    fn execute(&self, args: &HashMap<String, String>) -> Result<String, ToolError> {
        let path = args
            .get("path")
            .ok_or_else(|| ToolError::InvalidArguments {
                tool: "write_file".to_string(),
                reason: "Missing 'path' argument".to_string(),
            })?;
        let content = args
            .get("content")
            .ok_or_else(|| ToolError::InvalidArguments {
                tool: "write_file".to_string(),
                reason: "Missing 'content' argument".to_string(),
            })?;

        std::fs::write(path, content).map_err(|e| ToolError::ExecutionFailed {
            tool: "write_file".to_string(),
            source: Box::new(e),
        })?;

        Ok(format!("Wrote {} bytes to {path}", content.len()))
    }
}

export!(CoderAgent);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_role_conversion() {
        assert!(matches!(
            convert_role(Role::System),
            brio::ai::inference::Role::System
        ));
        assert!(matches!(
            convert_role(Role::User),
            brio::ai::inference::Role::User
        ));
        assert!(matches!(
            convert_role(Role::Assistant),
            brio::ai::inference::Role::Assistant
        ));
        assert!(matches!(
            convert_role(Role::Tool),
            brio::ai::inference::Role::Assistant
        ));
    }

    #[test]
    fn test_write_file_tool() {
        let tool = WriteFileTool;
        let mut args = HashMap::new();
        args.insert("path".to_string(), "/tmp/test_coder_file.txt".to_string());
        args.insert("content".to_string(), "Hello from coder agent".to_string());

        let result = tool.execute(&args).unwrap();
        assert!(result.contains("bytes"));

        // Cleanup
        let _ = std::fs::remove_file("/tmp/test_coder_file.txt");
    }

    #[test]
    fn test_coder_agent_implements_standard_agent() {
        let agent = CoderAgent;
        let context = TaskContext::new("test", "Test task");
        let config = StandardAgentConfig::default();
        let tools = agent.create_tool_registry(&AgentConfig::default());

        let prompt = agent.build_prompt(&context, &tools, &config);
        assert!(prompt.contains("expert software engineer"));
        assert!(prompt.contains("Test task"));
    }
}
