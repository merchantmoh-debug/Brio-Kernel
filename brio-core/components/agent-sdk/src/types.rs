//! Core types for agent operations.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Represents a message in the conversation history.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Message {
    /// The role of the message sender.
    pub role: Role,
    /// The content of the message.
    pub content: String,
    /// Optional metadata (e.g., tool call ID, timestamp).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<HashMap<String, String>>,
}

impl Message {
    /// Creates a new message.
    pub fn new(role: Role, content: impl Into<String>) -> Self {
        Self {
            role,
            content: content.into(),
            metadata: None,
        }
    }

    /// Creates a new system message.
    pub fn system(content: impl Into<String>) -> Self {
        Self::new(Role::System, content)
    }

    /// Creates a new user message.
    pub fn user(content: impl Into<String>) -> Self {
        Self::new(Role::User, content)
    }

    /// Creates a new assistant message.
    pub fn assistant(content: impl Into<String>) -> Self {
        Self::new(Role::Assistant, content)
    }

    /// Creates a new tool message.
    #[must_use]
    pub fn tool(content: impl Into<String>) -> Self {
        Self::new(Role::Tool, content)
    }

    /// Adds metadata to the message.
    #[must_use]
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata
            .get_or_insert_with(HashMap::new)
            .insert(key.into(), value.into());
        self
    }
}

/// Identifies the role of a message sender.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    /// System instruction or context.
    System,
    /// User input or query.
    User,
    /// Assistant/AI response.
    Assistant,
    /// Tool execution result.
    Tool,
}

impl std::fmt::Display for Role {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Role::System => write!(f, "system"),
            Role::User => write!(f, "user"),
            Role::Assistant => write!(f, "assistant"),
            Role::Tool => write!(f, "tool"),
        }
    }
}

/// Response from the inference API.
#[derive(Clone, Debug)]
pub struct InferenceResponse {
    /// The content of the response.
    pub content: String,
    /// The model used for inference.
    pub model: String,
    /// Number of tokens used (if available).
    pub tokens_used: Option<u32>,
    /// Completion reason (e.g., "stop", "length", "`tool_calls`").
    pub finish_reason: Option<String>,
}

/// Task context containing task metadata and parameters.
#[derive(Clone, Debug, Default)]
pub struct TaskContext {
    /// Unique identifier for the task.
    pub task_id: String,
    /// Description of what needs to be done.
    pub description: String,
    /// List of file paths to include in context.
    pub input_files: Vec<String>,
    /// Optional metadata for the task.
    pub metadata: Option<serde_json::Value>,
}

impl TaskContext {
    /// Creates a new task context.
    pub fn new(task_id: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            task_id: task_id.into(),
            description: description.into(),
            input_files: Vec::new(),
            metadata: None,
        }
    }

    /// Adds input files to the context.
    #[must_use]
    pub fn with_files(mut self, files: Vec<impl Into<String>>) -> Self {
        self.input_files = files.into_iter().map(std::convert::Into::into).collect();
        self
    }

    /// Adds metadata to the context.
    #[must_use]
    pub fn with_metadata(mut self, metadata: serde_json::Value) -> Self {
        self.metadata = Some(metadata);
        self
    }

    /// Checks if the context has input files.
    #[must_use]
    pub fn has_input_files(&self) -> bool {
        !self.input_files.is_empty()
    }

    /// Returns the number of input files.
    #[must_use]
    pub fn file_count(&self) -> usize {
        self.input_files.len()
    }
}

/// Tool invocation parsed from agent response.
///
/// Represents a tool call extracted from an agent's response, including the tool name,
/// arguments, and position in the response for ordering multiple tool calls.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolInvocation {
    /// Name of the tool to invoke.
    pub name: String,
    /// Arguments for the tool.
    pub args: HashMap<String, String>,
    /// Position in the response text (for ordering).
    pub position: usize,
}

/// Result of tool execution.
///
/// Contains the outcome of a tool invocation, including success status,
/// output content, and execution duration.
#[derive(Debug)]
pub struct ToolResult {
    /// Whether the execution was successful.
    pub success: bool,
    /// Output or result of the execution.
    pub output: String,
    /// Execution time.
    pub duration: std::time::Duration,
}

/// Execution result from processing all tools in a response.
#[derive(Debug)]
pub struct ExecutionResult {
    /// Combined output from all tool executions.
    pub output: String,
    /// Whether the task is marked as complete.
    pub is_complete: bool,
    /// Final summary if task is complete.
    pub summary: Option<String>,
    /// Individual tool results.
    pub tool_results: Vec<ToolResult>,
}

impl ExecutionResult {
    /// Creates a new execution result.
    #[must_use]
    pub fn new(output: String) -> Self {
        Self {
            output,
            is_complete: false,
            summary: None,
            tool_results: Vec::new(),
        }
    }

    /// Marks the result as complete with a summary.
    #[must_use]
    pub fn complete(mut self, summary: impl Into<String>) -> Self {
        self.is_complete = true;
        self.summary = Some(summary.into());
        self
    }

    /// Adds a tool result to the execution.
    ///
    /// # Arguments
    ///
    /// * `result` - The tool execution result to add.
    ///
    /// # Returns
    ///
    /// The modified `ExecutionResult` with the tool result appended.
    #[must_use]
    pub fn add_tool_result(mut self, result: ToolResult) -> Self {
        self.tool_results.push(result);
        self
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)] // Tests should fail fast on unrecoverable errors
mod tests {
    use super::*;

    #[test]
    fn test_message_creation() {
        let msg = Message::user("Hello");
        assert_eq!(msg.role, Role::User);
        assert_eq!(msg.content, "Hello");
    }

    #[test]
    fn test_message_with_metadata() {
        let msg = Message::assistant("Response")
            .with_metadata("tool_call_id", "123")
            .with_metadata("timestamp", "2024-01-01");

        let meta = msg.metadata.expect("metadata should be present");
        assert_eq!(meta.get("tool_call_id"), Some(&"123".to_string()));
        assert_eq!(meta.get("timestamp"), Some(&"2024-01-01".to_string()));
    }

    #[test]
    fn test_task_context_builder() {
        let ctx = TaskContext::new("task-1", "Do something")
            .with_files(vec!["file1.rs", "file2.rs"])
            .with_metadata(serde_json::json!({"priority": "high"}));

        assert_eq!(ctx.task_id, "task-1");
        assert!(ctx.has_input_files());
        assert_eq!(ctx.file_count(), 2);
    }

    #[test]
    fn test_role_display() {
        assert_eq!(Role::System.to_string(), "system");
        assert_eq!(Role::User.to_string(), "user");
        assert_eq!(Role::Assistant.to_string(), "assistant");
        assert_eq!(Role::Tool.to_string(), "tool");
    }
}
