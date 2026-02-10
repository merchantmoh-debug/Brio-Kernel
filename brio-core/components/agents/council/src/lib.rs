//! Council Agent - Strategic planning and oversight
#![allow(missing_docs)]

use agent_sdk::agent::parsers::{
    create_done_parser, create_list_parser, create_read_parser, create_write_parser,
};
use agent_sdk::agent::tools::{DoneTool, ListDirectoryTool, ReadFileTool};
use agent_sdk::agent::{
    StandardAgent, StandardAgentConfig, handle_standard_event, run_standard_agent,
};
use agent_sdk::error::AgentError;
use agent_sdk::tools::ToolRegistry;
use agent_sdk::types::{InferenceResponse, Message, Role, TaskContext};
use agent_sdk::{AgentConfig, PromptBuilder, Tool, ToolError};
use std::borrow::Cow;
use std::collections::HashMap;
use wit_bindgen::generate;

// Generate WIT bindings at crate root level
generate!({
    world: "smart-agent",
    path: "../../../wit",
    skip: ["tool"],
    generate_all,
});

/// `CouncilAgent` implements strategic planning using the `StandardAgent` trait.
#[derive(Clone)]
pub struct CouncilAgent;

impl StandardAgent for CouncilAgent {
    const NAME: &'static str = "council-agent";

    fn build_prompt(
        &self,
        context: &TaskContext,
        tools: &ToolRegistry,
        _config: &StandardAgentConfig,
    ) -> String {
        // Get base config for prompt building
        let agent_config = AgentConfig::from_env().unwrap_or_default();
        PromptBuilder::build_council_agent(context, tools, &agent_config)
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

        // Council has write capability for strategic plans
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

        let response = brio::ai::inference::chat(model, &wit_messages).map_err(|e| {
            AgentError::Inference(agent_sdk::InferenceError::ApiError(format!("{e:?}")))
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

impl exports::brio::core::agent_runner::Guest for CouncilAgent {
    fn run(context: exports::brio::core::agent_runner::TaskContext) -> Result<String, String> {
        let task_context = convert_wit_context(&context);
        let config = StandardAgentConfig::default();

        run_standard_agent(&CouncilAgent, &task_context, &config)
            .map_err(|e| format!("Agent execution failed: {e}"))
    }
}

impl exports::brio::core::event_handler::Guest for CouncilAgent {
    fn handle_event(topic: String, data: exports::brio::core::event_handler::Payload) {
        let data_str = match &data {
            exports::brio::core::event_handler::Payload::Json(s) => s.clone(),
            exports::brio::core::event_handler::Payload::Binary(_) => "[Binary data]".to_string(),
        };

        handle_standard_event(Self::NAME, &topic, &data_str);
    }
}

fn convert_wit_context(context: &exports::brio::core::agent_runner::TaskContext) -> TaskContext {
    TaskContext::new(&context.task_id, &context.description).with_files(context.input_files.clone())
}

fn convert_role(role: Role) -> brio::ai::inference::Role {
    match role {
        Role::System => brio::ai::inference::Role::System,
        Role::User => brio::ai::inference::Role::User,
        Role::Assistant | Role::Tool => brio::ai::inference::Role::Assistant,
    }
}

// WriteFileTool implementation for council agent
struct WriteFileTool;

impl Tool for WriteFileTool {
    fn name(&self) -> Cow<'static, str> {
        Cow::Borrowed("write_file")
    }

    fn description(&self) -> Cow<'static, str> {
        Cow::Borrowed(
            r#"<write_file path="path/to/file">content</write_file> - Write strategic plans and documentation"#,
        )
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

        Ok(format!("Wrote {} bytes to {}", content.len(), path))
    }
}

export!(CouncilAgent);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_tools_registered() {
        let agent = CouncilAgent;
        let config = AgentConfig::default();
        let registry = agent.create_tool_registry(&config);
        let tools = registry.available_tools();

        // Verify all 4 tools are registered
        assert!(tools.contains(&"done"), "done tool should be registered");
        assert!(
            tools.contains(&"read_file"),
            "read_file tool should be registered"
        );
        assert!(tools.contains(&"ls"), "ls tool should be registered");
        assert!(
            tools.contains(&"write_file"),
            "write_file tool should be registered"
        );
        assert_eq!(tools.len(), 4, "should have exactly 4 tools");
    }

    #[test]
    fn test_council_has_write_capability() {
        let agent = CouncilAgent;
        let config = AgentConfig::default();
        let registry = agent.create_tool_registry(&config);
        let tools = registry.available_tools();

        // Council agent should have write capability for strategic plans
        assert!(
            tools.contains(&"write_file"),
            "council agent should have write_file tool for strategic plans"
        );
    }

    #[test]
    fn test_write_file_tool_execution() -> Result<(), ToolError> {
        let tool = WriteFileTool;
        let mut args = HashMap::new();
        args.insert("path".to_string(), "/tmp/test_council_file.txt".to_string());
        args.insert("content".to_string(), "Strategic plan content".to_string());

        let result = tool.execute(&args)?;
        assert!(result.contains("bytes"));

        // Cleanup
        let _ = std::fs::remove_file("/tmp/test_council_file.txt");
        Ok(())
    }

    #[test]
    fn test_write_file_tool_missing_path() {
        let tool = WriteFileTool;
        let mut args = HashMap::new();
        args.insert("content".to_string(), "content".to_string());

        let result = tool.execute(&args);
        assert!(result.is_err());
    }

    #[test]
    fn test_council_agent_implements_standard_agent() {
        let agent = CouncilAgent;
        let context = TaskContext::new("test", "Test strategic planning");
        let config = StandardAgentConfig::default();
        let tools = agent.create_tool_registry(&AgentConfig::default());

        let prompt = agent.build_prompt(&context, &tools, &config);
        assert!(prompt.contains("Test strategic planning"));
    }
}
