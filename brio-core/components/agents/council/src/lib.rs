//! Council Agent - A strategic planning and oversight agent.
//!
//! This agent uses the agent-sdk to provide strategic planning capabilities.
//! Note: This agent has minimal tools as it's focused on planning, not execution.

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

/// CouncilAgent implements the agent-runner and event-handler interfaces.
pub struct CouncilAgent;

impl exports::brio::core::agent_runner::Guest for CouncilAgent {
    fn run(context: exports::brio::core::agent_runner::TaskContext) -> Result<String, String> {
        // Convert WIT context to SDK TaskContext
        let task_context = convert_wit_context(&context);

        // Load configuration
        let config_raw =
            AgentConfig::from_env().map_err(|e| format!("Configuration error: {}", e))?;
        let config = config_raw
            .validate()
            .map_err(|e| format!("Validation error: {}", e))?;

        // Create tool registry with minimal tools (planning-focused)
        let tools = create_tool_registry(&config);

        // Build the agent engine
        let mut engine = AgentEngineBuilder::new()
            .tools(tools)
            .config(config.clone())
            .build(&task_context)
            .map_err(|e| format!("Failed to initialize agent: {}", e))?;

        // Build system prompt
        let system_prompt = PromptBuilder::build_council_agent(
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

impl exports::brio::core::event_handler::Guest for CouncilAgent {
    fn handle_event(topic: String, data: exports::brio::core::event_handler::Payload) {
        let data_str = match &data {
            exports::brio::core::event_handler::Payload::Json(s) => s.clone(),
            exports::brio::core::event_handler::Payload::Binary(_) => "[Binary data]".to_string(),
        };

        brio::core::logging::log(
            brio::core::logging::Level::Info,
            "council-agent",
            &format!("Received event on topic '{}': {}", topic, data_str),
        );
    }
}

impl CouncilAgent {
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

/// Creates a tool registry with minimal tools for council agent.
/// NOTE: Council agent only has done tool as it's focused on strategic planning.
fn create_tool_registry(_config: &AgentConfig) -> ToolRegistry {
    let mut registry = ToolRegistry::new();

    // Only register done tool for council agent
    registry.register("done", Box::new(DoneTool), create_done_parser());

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

// Tool implementations
use agent_sdk::Tool;
use agent_sdk::error::ToolError;

struct DoneTool;

impl Tool for DoneTool {
    fn name(&self) -> &str {
        "done"
    }

    fn description(&self) -> &str {
        r#"<done>summary of completion</done> - Mark task as complete with strategic plan"#
    }

    fn execute(&self, _args: HashMap<String, String>) -> Result<String, ToolError> {
        Ok("Strategic plan complete".to_string())
    }
}

export!(CouncilAgent);

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
