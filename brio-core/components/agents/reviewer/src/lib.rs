//! Reviewer Agent - A specialized agent for code review and analysis.
//!
//! This agent uses the agent-sdk StandardAgent trait for a streamlined implementation.
//!
//! # Safety Feature: Read-Only Operation
//!
//! This agent intentionally does NOT include the `write_file` tool. This is a deliberate
//! safety feature to prevent the reviewer from accidentally modifying code during review.
//! The reviewer can read files and provide feedback, but cannot make changes.

#![allow(missing_docs)]

use agent_sdk::{
    AgentConfig, AgentError, PromptBuilder,
    agent::{
        StandardAgent, StandardAgentConfig,
        parsers::{create_done_parser, create_list_parser, create_read_parser},
        run_standard_agent,
        tools::{DoneTool, ListDirectoryTool, ReadFileTool},
    },
    tools::ToolRegistry,
    types::{InferenceResponse, Message, Role, TaskContext},
};

// Generate WIT bindings at crate root level
wit_bindgen::generate!({
    world: "smart-agent",
    path: "../../../wit",
    skip: ["tool"],
    generate_all,
});

/// ReviewerAgent implements the StandardAgent trait for code review.
///
/// # Safety
/// This agent does NOT include write capabilities - it is read-only by design.
/// This prevents accidental code modifications during review.
#[derive(Clone)]
pub struct ReviewerAgent;

impl StandardAgent for ReviewerAgent {
    const NAME: &'static str = "reviewer-agent";

    fn build_prompt(
        &self,
        context: &TaskContext,
        tools: &ToolRegistry,
        _config: &StandardAgentConfig,
    ) -> String {
        // Use the SDK's built-in reviewer prompt builder
        PromptBuilder::build_reviewer_agent(context, tools, &AgentConfig::default())
    }

    fn create_tool_registry(&self, config: &AgentConfig) -> ToolRegistry {
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

        // SAFETY: Reviewer does NOT have write tool - this is intentional
        // to prevent accidental code modifications during review.

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
            AgentError::Inference(agent_sdk::error::InferenceError::ApiError(format!("{e:?}")))
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

impl exports::brio::core::agent_runner::Guest for ReviewerAgent {
    fn run(context: exports::brio::core::agent_runner::TaskContext) -> Result<String, String> {
        // Convert WIT context to SDK TaskContext
        let task_context = TaskContext::new(&context.task_id, &context.description)
            .with_files(context.input_files);

        // Run the standard agent
        let config = StandardAgentConfig::default();

        run_standard_agent(&ReviewerAgent, &task_context, &config)
            .map_err(|e| format!("Agent execution failed: {e}"))
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
            &format!("Received event on topic '{topic}': {data_str}"),
        );
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

export!(ReviewerAgent);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reviewer_agent_implements_standard_agent() {
        let agent = ReviewerAgent;
        let context = TaskContext::new("test", "Review this code");
        let config = StandardAgentConfig::default();
        let tools = ToolRegistry::new();

        let prompt = agent.build_prompt(&context, &tools, &config);
        assert!(prompt.contains("Code Reviewer"));
        assert!(prompt.contains("Review this code"));
    }

    #[test]
    fn test_tool_registry_excludes_write_tool() {
        let agent = ReviewerAgent;
        let config = AgentConfig::default();
        let registry = agent.create_tool_registry(&config);
        let tools = registry.available_tools();

        // Verify read tool is present
        assert!(tools.contains(&"read_file"));
        // Verify list tool is present
        assert!(tools.contains(&"ls"));
        // Verify done tool is present
        assert!(tools.contains(&"done"));
        // SAFETY: Verify write tool is NOT present (read-only agent)
        assert!(!tools.contains(&"write_file"));
    }

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
}
