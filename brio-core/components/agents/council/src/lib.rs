//! Council Agent - Strategic planning and oversight
#![allow(missing_docs)]

use agent_sdk::agent::{
    handle_standard_event, run_standard_agent, StandardAgent, StandardAgentConfig,
};
use agent_sdk::error::AgentError;
use agent_sdk::tools::ToolRegistry;
use agent_sdk::types::{InferenceResponse, Message, Role, TaskContext};
use agent_sdk::{AgentConfig, PromptBuilder};
use regex::Regex;
use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::OnceLock;
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

    fn create_tool_registry(&self, _config: &AgentConfig) -> ToolRegistry {
        let mut registry = ToolRegistry::new();
        // Council uses only 'done' tool (from base)
        registry.register("done", Box::new(DoneTool), create_done_parser());
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

static DONE_REGEX: OnceLock<Regex> = OnceLock::new();

fn create_done_parser() -> agent_sdk::tools::ToolParser {
    let regex = DONE_REGEX.get_or_init(|| {
        Regex::new(r"<done>\s*(.*?)\s*</done>").expect("DONE_REGEX should be valid")
    });
    agent_sdk::tools::ToolParser::from_regex(regex, |caps: &regex::Captures| {
        let mut args = HashMap::new();
        args.insert("summary".to_string(), caps[1].to_string());
        args
    })
}

use agent_sdk::error::ToolError;
use agent_sdk::Tool;

struct DoneTool;

impl Tool for DoneTool {
    fn name(&self) -> Cow<'static, str> {
        Cow::Borrowed("done")
    }

    fn description(&self) -> Cow<'static, str> {
        Cow::Borrowed(
            r"<done>summary of completion</done> - Mark task as complete with strategic plan",
        )
    }

    fn execute(&self, _args: &HashMap<String, String>) -> Result<String, ToolError> {
        Ok("Strategic plan complete".to_string())
    }
}

export!(CouncilAgent);
