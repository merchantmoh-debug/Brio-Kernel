//! Smart Agent - A general-purpose autonomous agent.
//!
//! This agent uses the agent-sdk to provide comprehensive software engineering
//! capabilities including code writing, reading, and shell command execution.

// WIT bindings generate many undocumented items - this is expected for auto-generated code
#![allow(missing_docs)]

use agent_sdk::{
    AgentConfig, AgentError, PromptBuilder,
    agent::tools::{DoneTool, ListDirectoryTool, ReadFileTool},
    agent::{StandardAgent, StandardAgentConfig, run_standard_agent},
    tools::{Tool, ToolParser, ToolRegistry},
    types::{InferenceResponse, Message, Role, TaskContext},
};
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

/// `SmartAgent` implements the `StandardAgent` trait for general-purpose tasks.
#[derive(Clone)]
pub struct SmartAgent;

impl StandardAgent for SmartAgent {
    const NAME: &'static str = "smart-agent";

    fn build_prompt(
        &self,
        context: &TaskContext,
        tools: &ToolRegistry,
        _config: &StandardAgentConfig,
    ) -> String {
        // Use the default AgentConfig for prompt building
        let agent_config = AgentConfig::default();
        PromptBuilder::build_smart_agent(context, tools, &agent_config)
    }

    fn create_tool_registry(&self, config: &AgentConfig) -> ToolRegistry {
        let mut registry = ToolRegistry::new();

        // Always register core tools
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

        // Conditionally register write tool
        if config.tool_config.enable_write {
            registry.register("write_file", Box::new(WriteFileTool), create_write_parser());
        }

        // Conditionally register shell tool with allowlist
        if config.tool_config.enable_shell {
            registry.register(
                "shell",
                Box::new(ShellTool::new(config.shell_allowlist.clone())),
                create_shell_parser(),
            );
        }

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

impl exports::brio::core::agent_runner::Guest for SmartAgent {
    fn run(context: exports::brio::core::agent_runner::TaskContext) -> Result<String, String> {
        // Convert WIT context to SDK TaskContext
        let task_context = TaskContext::new(&context.task_id, &context.description)
            .with_files(context.input_files.clone());

        // Create standard agent configuration
        let config = StandardAgentConfig::default();

        // Run the standard agent
        run_standard_agent(&SmartAgent, &task_context, &config)
            .map_err(|e| format!("Agent execution failed: {e}"))
    }
}

impl exports::brio::core::event_handler::Guest for SmartAgent {
    fn handle_event(topic: String, data: exports::brio::core::event_handler::Payload) {
        let data_str = match &data {
            exports::brio::core::event_handler::Payload::Json(s) => s.clone(),
            exports::brio::core::event_handler::Payload::Binary(_) => "[Binary data]".to_string(),
        };

        brio::core::logging::log(
            brio::core::logging::Level::Info,
            "smart-agent",
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

// Tool parsers - using OnceLock for lazy regex compilation
static DONE_REGEX: OnceLock<Regex> = OnceLock::new();

fn create_done_parser() -> ToolParser {
    let regex = DONE_REGEX.get_or_init(|| {
        Regex::new(r"<done>\s*(.*?)\s*</done>").expect("DONE_REGEX should be valid")
    });
    ToolParser::from_regex(regex, |caps: &regex::Captures| {
        let mut args = HashMap::new();
        args.insert("summary".to_string(), caps[1].to_string());
        args
    })
}

static READ_REGEX: OnceLock<Regex> = OnceLock::new();

fn create_read_parser() -> ToolParser {
    let regex = READ_REGEX.get_or_init(|| {
        Regex::new(r#"<read_file\s+path="([^"]+)"\s*/?>"#).expect("READ_REGEX should be valid")
    });
    ToolParser::from_regex(regex, |caps: &regex::Captures| {
        let mut args = HashMap::new();
        args.insert("path".to_string(), caps[1].to_string());
        args
    })
}

static LIST_REGEX: OnceLock<Regex> = OnceLock::new();

fn create_list_parser() -> ToolParser {
    let regex = LIST_REGEX.get_or_init(|| {
        Regex::new(r#"<ls\s+path="([^"]+)"\s*/?>"#).expect("LIST_REGEX should be valid")
    });
    ToolParser::from_regex(regex, |caps: &regex::Captures| {
        let mut args = HashMap::new();
        args.insert("path".to_string(), caps[1].to_string());
        args
    })
}

static WRITE_REGEX: OnceLock<Regex> = OnceLock::new();

fn create_write_parser() -> ToolParser {
    let regex = WRITE_REGEX.get_or_init(|| {
        Regex::new(r#"<write_file\s+path="([^"]+)">\s*(.*?)\s*</write_file>"#)
            .expect("WRITE_REGEX should be valid")
    });
    ToolParser::from_regex(regex, |caps: &regex::Captures| {
        let mut args = HashMap::new();
        args.insert("path".to_string(), caps[1].to_string());
        args.insert("content".to_string(), caps[2].to_string());
        args
    })
}

static SHELL_REGEX: OnceLock<Regex> = OnceLock::new();

fn create_shell_parser() -> ToolParser {
    let regex = SHELL_REGEX.get_or_init(|| {
        Regex::new(r"<shell>\s*(.*?)\s*</shell>").expect("SHELL_REGEX should be valid")
    });
    ToolParser::from_regex(regex, |caps: &regex::Captures| {
        let mut args = HashMap::new();
        args.insert("command".to_string(), caps[1].to_string());
        args
    })
}

// Smart-agent-specific tool implementations
use agent_sdk::error::ToolError;

struct WriteFileTool;

impl Tool for WriteFileTool {
    fn name(&self) -> Cow<'static, str> {
        Cow::Borrowed("write_file")
    }

    fn description(&self) -> Cow<'static, str> {
        Cow::Borrowed(
            r#"<write_file path="path/to/file">content</write_file> - Write content to a file"#,
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

        Ok(format!("Wrote {} bytes to {path}", content.len()))
    }
}

struct ShellTool {
    allowlist: Vec<String>,
}

impl ShellTool {
    fn new(allowlist: Vec<String>) -> Self {
        Self { allowlist }
    }
}

impl Tool for ShellTool {
    fn name(&self) -> Cow<'static, str> {
        Cow::Borrowed("shell")
    }

    fn description(&self) -> Cow<'static, str> {
        Cow::Borrowed(r"<shell>command</shell> - Execute a shell command")
    }

    fn execute(&self, args: &HashMap<String, String>) -> Result<String, ToolError> {
        let command = args
            .get("command")
            .ok_or_else(|| ToolError::InvalidArguments {
                tool: "shell".to_string(),
                reason: "Missing 'command' argument".to_string(),
            })?;

        // Validate command against allowlist
        agent_sdk::validate_shell_command(command, &self.allowlist).map_err(|e| {
            ToolError::Blocked {
                tool: "shell".to_string(),
                reason: e.to_string(),
            }
        })?;

        let output = std::process::Command::new("sh")
            .arg("-c")
            .arg(command)
            .output()
            .map_err(|e| ToolError::ExecutionFailed {
                tool: "shell".to_string(),
                source: Box::new(e),
            })?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if !output.status.success() {
            return Err(ToolError::ExecutionFailed {
                tool: "shell".to_string(),
                source: Box::new(std::io::Error::other(format!(
                    "Exit code {:?}: {stderr}",
                    output.status.code()
                ))),
            });
        }

        Ok(stdout.to_string())
    }
}

export!(SmartAgent);

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
    fn test_standard_agent_trait() {
        let agent = SmartAgent;
        let context = TaskContext::new("test", "Do something");
        let config = StandardAgentConfig::default();
        let tools = ToolRegistry::new();

        let prompt = agent.build_prompt(&context, &tools, &config);
        assert!(prompt.contains("Smart Agent"));
        assert!(prompt.contains("Do something"));
    }

    #[test]
    fn test_tool_registry_with_all_tools() {
        let agent = SmartAgent;
        let mut config = AgentConfig::default();
        config.tool_config.enable_write = true;
        config.tool_config.enable_shell = true;
        config.shell_allowlist = vec!["ls".to_string(), "cat".to_string()];

        let registry = agent.create_tool_registry(&config);
        let tools = registry.available_tools();

        assert!(tools.contains(&"done"));
        assert!(tools.contains(&"read_file"));
        assert!(tools.contains(&"ls"));
        assert!(tools.contains(&"write_file"));
        assert!(tools.contains(&"shell"));
    }

    #[test]
    fn test_tool_registry_without_conditional_tools() {
        let agent = SmartAgent;
        let mut config = AgentConfig::default();
        config.tool_config.enable_write = false;
        config.tool_config.enable_shell = false;

        let registry = agent.create_tool_registry(&config);
        let tools = registry.available_tools();

        assert!(tools.contains(&"done"));
        assert!(tools.contains(&"read_file"));
        assert!(tools.contains(&"ls"));
        assert!(!tools.contains(&"write_file"));
        assert!(!tools.contains(&"shell"));
    }
}
