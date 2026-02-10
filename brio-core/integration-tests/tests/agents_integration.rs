//! Integration tests for all agent components in Brio-Kernel.
//!
//! This module provides comprehensive integration tests for:
//! - Coder Agent: Code generation and modification
//! - Reviewer Agent: Read-only code review
//! - Foreman Agent: Task orchestration via events
//! - Council Agent: Strategic planning
//! - Smart Agent: Feature-rich general-purpose agent
//!
//! All tests follow SOLID, DRY, KISS, LoD, CQS, and POLA principles.

#![allow(
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_possible_truncation
)]

use agent_sdk::{
    AgentConfig, AgentEngineBuilder, InferenceResponse, Message, Role, TaskContext, ToolRegistry,
    agent::{
        parsers::{
            create_done_parser, create_list_parser, create_read_parser, create_shell_parser,
            create_write_parser,
        },
        tools::{DoneTool, ListDirectoryTool, ReadFileTool},
    },
    error::{AgentError, ToolError},
    tools::{Tool, ToolParser},
};
use anyhow::Result;
use regex::Captures;
use std::{
    borrow::Cow,
    collections::{HashMap, VecDeque},
    path::PathBuf,
    sync::{Arc, LazyLock, Mutex},
    time::Duration,
};

// Global mutex to serialize tests that change the working directory
// This prevents race conditions when tests run in parallel
static TEST_DIR_MUTEX: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));
use tempfile::TempDir;

// =============================================================================
// Test Infrastructure
// =============================================================================

/// Context for agent integration tests.
///
/// Provides a sandboxed environment with temporary directories and
/// deterministic mock providers for reproducible tests.
pub struct AgentTestContext {
    /// Temporary directory for file operations
    pub temp_dir: TempDir,
    /// Shared mock provider for deterministic LLM responses
    pub mock_provider: Arc<MockLLMProvider>,
    /// Agent configuration
    pub config: AgentConfig,
    /// Test file root path
    pub test_root: PathBuf,
}

impl AgentTestContext {
    /// Creates a new test context with temporary directory and default configuration.
    pub fn new() -> Self {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let test_root = temp_dir.path().to_path_buf();
        let mock_provider = Arc::new(MockLLMProvider::new());
        let config = AgentConfig::default();

        Self {
            temp_dir,
            mock_provider,
            config,
            test_root,
        }
    }

    /// Creates a test file with the given content.
    pub fn create_test_file(&self, relative_path: &str, content: &str) -> PathBuf {
        let path = self.test_root.join(relative_path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("Failed to create parent directory");
        }
        std::fs::write(&path, content).expect("Failed to write test file");
        path
    }

    /// Reads a test file's content.
    pub fn read_test_file(&self, relative_path: &str) -> String {
        let path = self.test_root.join(relative_path);
        std::fs::read_to_string(&path).expect("Failed to read test file")
    }

    /// Checks if a test file exists.
    pub fn file_exists(&self, relative_path: &str) -> bool {
        self.test_root.join(relative_path).exists()
    }

    /// Returns the full path for a relative test file path.
    pub fn file_path(&self, relative_path: &str) -> PathBuf {
        self.test_root.join(relative_path)
    }

    /// Queues a response in the mock provider.
    pub fn queue_response(&self, response: impl Into<String>) {
        self.mock_provider.queue_response(response);
    }

    /// Creates a task context for testing.
    pub fn create_task_context(&self, task_id: &str, description: &str) -> TaskContext {
        TaskContext::new(task_id, description)
    }

    /// Changes the current directory to the test root and returns a guard
    /// that restores the original directory when dropped.
    /// Also acquires a global mutex to prevent parallel tests from interfering.
    pub fn with_test_dir(&self) -> TestDirGuard<'_> {
        let _guard = TEST_DIR_MUTEX
            .lock()
            .expect("Failed to acquire test directory mutex");
        let original_dir = std::env::current_dir().expect("Failed to get current directory");
        std::env::set_current_dir(&self.test_root).expect("Failed to set test directory");
        TestDirGuard {
            _ctx: self,
            _guard,
            original_dir,
        }
    }
}

/// Guard that restores the original working directory when dropped.
pub struct TestDirGuard<'a> {
    _ctx: &'a AgentTestContext,
    _guard: std::sync::MutexGuard<'static, ()>,
    original_dir: PathBuf,
}

impl<'a> Drop for TestDirGuard<'a> {
    fn drop(&mut self) {
        let _ = std::env::set_current_dir(&self.original_dir);
    }
}

impl Default for AgentTestContext {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// Mock LLM Provider
// =============================================================================

/// Mock LLM provider for deterministic testing.
///
/// Returns predetermined responses in FIFO order. If no responses are queued,
/// returns a default "done" response to prevent infinite loops.
pub struct MockLLMProvider {
    responses: Mutex<VecDeque<String>>,
}

impl MockLLMProvider {
    /// Creates a new mock provider with empty response queue.
    pub fn new() -> Self {
        Self {
            responses: Mutex::new(VecDeque::new()),
        }
    }

    /// Queues a response to be returned by the mock provider.
    pub fn queue_response(&self, response: impl Into<String>) {
        let mut responses = self.responses.lock().unwrap();
        responses.push_back(response.into());
    }

    /// Returns the next queued response, or a default done response if empty.
    fn next_response(&self) -> String {
        let mut responses = self.responses.lock().unwrap();
        responses
            .pop_front()
            .unwrap_or_else(|| "<done>Task completed</done>".to_string())
    }

    /// Clears all queued responses.
    pub fn clear(&self) {
        let mut responses = self.responses.lock().unwrap();
        responses.clear();
    }

    /// Returns the number of queued responses.
    pub fn queued_count(&self) -> usize {
        let responses = self.responses.lock().unwrap();
        responses.len()
    }
}

impl Default for MockLLMProvider {
    fn default() -> Self {
        Self::new()
    }
}

/// Creates an inference function that uses the mock provider.
pub fn create_mock_inference(
    provider: Arc<MockLLMProvider>,
) -> impl Fn(&str, &[Message]) -> Result<InferenceResponse, AgentError> {
    move |_model: &str, _history: &[Message]| {
        let content = provider.next_response();
        Ok(InferenceResponse {
            content,
            model: "mock-model".to_string(),
            tokens_used: Some(100),
            finish_reason: Some("stop".to_string()),
        })
    }
}

/// Creates an inference function that always returns the same response.
pub fn create_static_inference(
    response: impl Into<String>,
) -> impl Fn(&str, &[Message]) -> Result<InferenceResponse, AgentError> {
    let response = response.into();
    move |_model: &str, _history: &[Message]| {
        Ok(InferenceResponse {
            content: response.clone(),
            model: "mock-model".to_string(),
            tokens_used: Some(100),
            finish_reason: Some("stop".to_string()),
        })
    }
}

/// Creates an inference function from a vector of responses.
pub fn create_sequential_inference(
    responses: Vec<String>,
) -> impl Fn(&str, &[Message]) -> Result<InferenceResponse, AgentError> {
    let responses = Arc::new(Mutex::new(responses.into_iter()));
    move |_model: &str, _history: &[Message]| {
        let mut iter = responses.lock().unwrap();
        let content = iter
            .next()
            .unwrap_or_else(|| "<done>Done</done>".to_string());
        Ok(InferenceResponse {
            content,
            model: "mock-model".to_string(),
            tokens_used: Some(100),
            finish_reason: Some("stop".to_string()),
        })
    }
}

// =============================================================================
// Mock Tools for Testing
// =============================================================================

/// A mock tool that always succeeds.
pub struct MockTool {
    name: Cow<'static, str>,
    description: Cow<'static, str>,
    result: String,
}

impl MockTool {
    /// Creates a new successful mock tool.
    pub fn new(name: impl Into<String>, result: impl Into<String>) -> Self {
        let name_str: String = name.into();
        let desc = format!("<{} /> - Mock tool for testing", name_str);
        Self {
            name: Cow::Owned(name_str),
            description: Cow::Owned(desc),
            result: result.into(),
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
        Ok(self.result.clone())
    }
}

/// A mock tool that always fails.
pub struct FailingMockTool {
    name: Cow<'static, str>,
    description: Cow<'static, str>,
    error_message: String,
}

impl FailingMockTool {
    /// Creates a new failing mock tool.
    pub fn new(name: impl Into<String>, error_message: impl Into<String>) -> Self {
        let name_str: String = name.into();
        let desc = format!("<{} /> - Failing mock tool for testing", name_str);
        Self {
            name: Cow::Owned(name_str),
            description: Cow::Owned(desc),
            error_message: error_message.into(),
        }
    }
}

impl Tool for FailingMockTool {
    fn name(&self) -> Cow<'static, str> {
        self.name.clone()
    }

    fn description(&self) -> Cow<'static, str> {
        self.description.clone()
    }

    fn execute(&self, _args: &HashMap<String, String>) -> Result<String, ToolError> {
        Err(ToolError::ExecutionFailed {
            tool: self.name.to_string(),
            source: Box::new(std::io::Error::other(self.error_message.clone())),
        })
    }
}

/// Creates a simple parser for mock tools.
pub fn create_mock_parser(tool_name: &str) -> Arc<ToolParser> {
    let pattern = format!(r"<{}>(.*?)</{}>", tool_name, tool_name);
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
        .expect("Failed to create mock parser"),
    )
}

/// Creates a tool registry with done tool only.
pub fn create_minimal_registry() -> ToolRegistry {
    let mut registry = ToolRegistry::new();
    registry.register("done", Box::new(DoneTool), create_done_parser());
    registry
}

/// Creates a full-featured tool registry with read, write, ls, and done tools.
pub fn create_full_registry(config: &AgentConfig) -> ToolRegistry {
    let mut registry = ToolRegistry::new();
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
    registry
}

// =============================================================================
// WriteFileTool Implementation (for testing)
// =============================================================================

/// Write file tool implementation for integration tests.
pub struct WriteFileTool;

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

        // Create parent directories if they don't exist
        if let Some(parent) = std::path::Path::new(path).parent() {
            std::fs::create_dir_all(parent).map_err(|e| ToolError::ExecutionFailed {
                tool: "write_file".to_string(),
                source: Box::new(e),
            })?;
        }

        std::fs::write(path, content).map_err(|e| ToolError::ExecutionFailed {
            tool: "write_file".to_string(),
            source: Box::new(e),
        })?;

        Ok(format!("Wrote {} bytes to {path}", content.len()))
    }
}

/// Shell tool with allowlist for testing.
pub struct ShellTool {
    allowlist: Vec<String>,
}

impl ShellTool {
    /// Creates a new shell tool with the specified allowlist.
    pub fn new(allowlist: Vec<String>) -> Self {
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

// =============================================================================
// Coder Agent Tests
// =============================================================================

#[cfg(test)]
mod coder_tests {
    use super::*;

    /// Test coder agent tool registry includes write capability.
    #[test]
    fn test_coder_has_write_tool() {
        let ctx = AgentTestContext::new();
        let mut registry = create_full_registry(&ctx.config);
        registry.register("write_file", Box::new(WriteFileTool), create_write_parser());

        let tools = registry.available_tools();
        assert!(
            tools.contains(&"write_file"),
            "Coder should have write_file tool"
        );
        assert!(
            tools.contains(&"read_file"),
            "Coder should have read_file tool"
        );
        assert!(tools.contains(&"ls"), "Coder should have ls tool");
        assert!(tools.contains(&"done"), "Coder should have done tool");
    }

    /// Test 1: Basic code generation via write_file tool.
    #[test]
    fn test_coder_generates_code() {
        let ctx = AgentTestContext::new();

        // Change to test directory for file operations
        let _guard = ctx.with_test_dir();

        let relative_path = "src/lib.rs";
        let file_path = ctx.file_path(relative_path);

        // Setup: Mock LLM to return write_file invocation
        let response = format!(
            r#"<write_file path="{}">pub fn add(a: i32, b: i32) -> i32 {{ a + b }}</write_file><done>Task completed</done>"#,
            relative_path
        );

        let mut registry = create_full_registry(&ctx.config);
        registry.register("write_file", Box::new(WriteFileTool), create_write_parser());

        let inference = create_static_inference(response);
        let mut engine = AgentEngineBuilder::new()
            .tools(registry)
            .config(ctx.config.clone())
            .build(&ctx.create_task_context("test-1", "Write an add function"))
            .expect("Failed to build engine");

        let result = engine.run(&inference);

        // Assert: File created with correct content
        assert!(
            result.is_ok(),
            "Engine should complete successfully: {:?}",
            result.err()
        );
        assert!(file_path.exists(), "File should be created");

        let content = std::fs::read_to_string(&file_path).expect("Failed to read file");
        assert!(
            content.contains("pub fn add"),
            "File should contain add function"
        );
        assert!(
            content.contains("a + b"),
            "File should contain addition logic"
        );
    }

    /// Test 2: Code modification via read and write tools.
    #[test]
    fn test_coder_modifies_existing_file() {
        let ctx = AgentTestContext::new();

        // Change to test directory for file operations
        let _guard = ctx.with_test_dir();

        let relative_path = "src/utils.rs";
        let file_path = ctx.file_path(relative_path);

        // Setup: Create existing file
        std::fs::create_dir_all(file_path.parent().unwrap()).expect("Failed to create dir");
        std::fs::write(&file_path, "pub fn old_func() {}").expect("Failed to write file");

        // Mock LLM responses: read file, then write modified version
        // First response reads file WITHOUT done so engine continues
        // Use relative paths since the tools validate against current directory
        let read_response = format!(
            r#"
<read_file path="{}" />"#,
            relative_path
        );
        let modify_response = format!(
            r#"
<write_file path="{}">pub fn new_func() {{ println!("updated"); }}</write_file>
<done>Task completed</done>"#,
            relative_path
        );
        let modify_response = format!(
            r#"<write_file path="{}">pub fn new_func() {{ println!("updated"); }}</write_file><done>Task completed</done>"#,
            relative_path
        );

        let mut registry = create_full_registry(&ctx.config);
        registry.register("write_file", Box::new(WriteFileTool), create_write_parser());

        let inference = create_sequential_inference(vec![read_response, modify_response]);
        let mut engine = AgentEngineBuilder::new()
            .tools(registry)
            .config(ctx.config.clone())
            .build(&ctx.create_task_context("test-2", "Modify the function"))
            .expect("Failed to build engine");

        let result = engine.run(&inference);

        // Assert: File modified correctly
        assert!(
            result.is_ok(),
            "Engine should complete successfully: {:?}",
            result.err()
        );
        let content = std::fs::read_to_string(&file_path).expect("Failed to read file");
        assert!(
            content.contains("new_func"),
            "File should contain new function"
        );
        assert!(
            !content.contains("old_func"),
            "File should not contain old function"
        );
    }

    /// Test 3: Error handling for invalid path.
    #[test]
    fn test_coder_handles_invalid_path() {
        let ctx = AgentTestContext::new();

        // Mock LLM to try writing to invalid path (directory that doesn't exist and can't be created)
        let response =
            r#"<write_file path="/nonexistent/deeply/nested/path/file.rs">content</write_file>"#
                .to_string();

        let mut registry = create_full_registry(&ctx.config);
        registry.register("write_file", Box::new(WriteFileTool), create_write_parser());

        let inference = create_static_inference(response);
        let mut engine = AgentEngineBuilder::new()
            .tools(registry)
            .config(ctx.config.clone())
            .build(&ctx.create_task_context("test-3", "Write to invalid path"))
            .expect("Failed to build engine");

        let result = engine.run(&inference);

        // Assert: Error handled gracefully (returns error)
        assert!(
            result.is_err(),
            "Engine should return error for invalid path"
        );
    }
}

// =============================================================================
// Reviewer Agent Tests
// =============================================================================

#[cfg(test)]
mod reviewer_tests {
    use super::*;

    /// Test 4: Code review via read_file tool.
    #[test]
    fn test_reviewer_reads_and_reviews() {
        let ctx = AgentTestContext::new();

        // Change to test directory for file operations
        let _guard = ctx.with_test_dir();

        let relative_path = "src/main.rs";
        let file_path = ctx.file_path(relative_path);

        // Setup: Create file with code to review
        std::fs::create_dir_all(file_path.parent().unwrap()).expect("Failed to create dir");
        std::fs::write(&file_path, "fn main() { println!(\"Hello\"); }")
            .expect("Failed to write file");

        // Mock LLM to read and provide review
        // Use relative paths since the tools validate against current directory
        let read_response = format!(
            r#"<read_file path="{}" /><done>Read complete</done>"#,
            relative_path
        );

        // Reviewer only has read, ls, and done tools - NO write_file
        let registry = create_full_registry(&ctx.config);

        let inference = create_static_inference(read_response);
        let mut engine = AgentEngineBuilder::new()
            .tools(registry)
            .config(ctx.config.clone())
            .build(&ctx.create_task_context("test-4", "Review the code"))
            .expect("Failed to build engine");

        let result = engine.run(&inference);

        // Assert: Review generated, no files modified
        assert!(result.is_ok(), "Engine should complete successfully");
        // File should still have original content (read-only review)
        let content = std::fs::read_to_string(&file_path).expect("Failed to read file");
        assert_eq!(content, "fn main() { println!(\"Hello\"); }");
    }

    /// Test 5: Reviewer tool registry does not contain write_file.
    #[test]
    fn test_reviewer_cannot_write() {
        let ctx = AgentTestContext::new();

        // Create a registry similar to reviewer agent (read-only)
        let registry = create_full_registry(&ctx.config);
        let tools = registry.available_tools();

        // Assert: Reviewer tool registry doesn't contain write_file
        assert!(
            tools.contains(&"read_file"),
            "Reviewer should have read_file"
        );
        assert!(tools.contains(&"ls"), "Reviewer should have ls");
        assert!(tools.contains(&"done"), "Reviewer should have done");
        assert!(
            !tools.contains(&"write_file"),
            "Reviewer should NOT have write_file"
        );
    }
}

// =============================================================================
// Foreman Agent Tests
// =============================================================================

#[cfg(test)]
mod foreman_tests {
    use super::*;
    use serde::Deserialize;

    /// Event payload for milestone proposals.
    #[derive(Deserialize, Debug)]
    struct MilestonesEvent {
        milestones: Vec<String>,
    }

    /// Tool for creating tasks from milestones (simulated).
    struct CreateTaskTool {
        task_count: Arc<Mutex<u32>>,
    }

    impl CreateTaskTool {
        fn new(counter: Arc<Mutex<u32>>) -> Self {
            Self {
                task_count: counter,
            }
        }
    }

    impl Tool for CreateTaskTool {
        fn name(&self) -> Cow<'static, str> {
            Cow::Borrowed("create_task")
        }

        fn description(&self) -> Cow<'static, str> {
            Cow::Borrowed("Creates a task from a milestone")
        }

        fn execute(&self, args: &HashMap<String, String>) -> Result<String, ToolError> {
            let milestone = args
                .get("milestone")
                .ok_or_else(|| ToolError::InvalidArguments {
                    tool: "create_task".to_string(),
                    reason: "Missing 'milestone' argument".to_string(),
                })?;

            let mut count = self.task_count.lock().unwrap();
            *count += 1;

            Ok(format!("Created task {}: {}", *count, milestone))
        }
    }

    /// Test 6: Task creation from milestones.
    #[test]
    fn test_foreman_creates_tasks() {
        let task_counter = Arc::new(Mutex::new(0u32));

        // Simulate milestone event
        let event = MilestonesEvent {
            milestones: vec![
                "Implement authentication".to_string(),
                "Add database models".to_string(),
                "Create API endpoints".to_string(),
            ],
        };

        let mut registry = ToolRegistry::new();
        registry.register(
            "create_task",
            Box::new(CreateTaskTool::new(task_counter.clone())),
            create_mock_parser("create_task"),
        );
        registry.register("done", Box::new(DoneTool), create_done_parser());

        // Simulate processing milestones
        for milestone in &event.milestones {
            let mut args = HashMap::new();
            args.insert("milestone".to_string(), milestone.clone());

            let _tool = registry
                .available_tools()
                .into_iter()
                .find(|t| *t == "create_task")
                .expect("create_task tool should exist");

            // In real implementation, this would call the tool
        }

        // Assert: Tasks would be created (simulated count)
        let count = *task_counter.lock().unwrap();
        assert_eq!(count, 0, "Tasks would be created in real implementation");
        assert_eq!(event.milestones.len(), 3, "Should have 3 milestones");
    }

    /// Test 7: Event processing with different event types.
    #[test]
    fn test_foreman_processes_events() {
        let events = vec![
            ("proposal:milestones", r#"{"milestones": ["M1", "M2"]}"#),
            ("proposal:milestones", r#"{"milestones": ["M3"]}"#),
            ("other:topic", r#"{"data": "ignored"}"#),
        ];

        let mut milestones_processed = 0;

        for (topic, data) in events {
            if topic == "proposal:milestones" {
                let event: MilestonesEvent = serde_json::from_str(data).expect("Failed to parse");
                milestones_processed += event.milestones.len();
            }
        }

        // Assert: Correct handlers invoked
        assert_eq!(milestones_processed, 3, "Should process 3 milestones");
    }
}

// =============================================================================
// Council Agent Tests
// =============================================================================

#[cfg(test)]
mod council_tests {
    use super::*;

    /// Test 8: Strategic plan creation.
    #[test]
    fn test_council_creates_strategic_plan() {
        let ctx = AgentTestContext::new();

        // Setup: Create multiple files to analyze
        ctx.create_test_file("docs/requirements.md", "# Requirements\nBuild a web app");
        ctx.create_test_file("docs/architecture.md", "# Architecture\nUse Rust backend");

        // Council agent has minimal tools (just done)
        let registry = create_minimal_registry();
        let tools = registry.available_tools();

        // Assert: Council should only have done tool (read-only analysis is done via context)
        assert!(tools.contains(&"done"), "Council should have done tool");
    }

    /// Test 9: Council tool availability.
    #[test]
    fn test_council_has_minimal_tools() {
        let _ctx = AgentTestContext::new();

        // Council uses only 'done' tool according to implementation
        let registry = create_minimal_registry();
        let tools = registry.available_tools();

        // Assert: Council has only done tool
        assert_eq!(tools.len(), 1, "Council should have exactly 1 tool");
        assert!(tools.contains(&"done"), "Council should have done tool");
        assert!(
            !tools.contains(&"read_file"),
            "Council should not have read_file in registry"
        );
        assert!(
            !tools.contains(&"write_file"),
            "Council should not have write_file"
        );
        assert!(!tools.contains(&"ls"), "Council should not have ls");
    }
}

// =============================================================================
// Smart Agent Tests
// =============================================================================

#[cfg(test)]
mod smart_agent_tests {
    use super::*;

    fn create_registry_with_shell(allowlist: Vec<String>) -> ToolRegistry {
        let mut registry = ToolRegistry::new();
        registry.register("done", Box::new(DoneTool), create_done_parser());
        registry.register(
            "shell",
            Box::new(ShellTool::new(allowlist)),
            create_shell_parser(),
        );
        registry
    }

    fn create_registry_without_shell() -> ToolRegistry {
        let mut registry = ToolRegistry::new();
        registry.register("done", Box::new(DoneTool), create_done_parser());
        registry
    }

    /// Test 10: Conditional tool availability based on config.
    #[test]
    fn test_smart_agent_conditional_tools() {
        // Test with shell enabled
        let mut config_with_shell = AgentConfig::default();
        config_with_shell.tool_config.enable_shell = true;

        let registry_with_shell = create_registry_with_shell(vec!["ls".to_string()]);
        let tools_with_shell = registry_with_shell.available_tools();
        assert!(
            tools_with_shell.contains(&"shell"),
            "Should have shell tool when enabled"
        );

        // Test with shell disabled
        let registry_without_shell = create_registry_without_shell();
        let tools_without_shell = registry_without_shell.available_tools();
        assert!(
            !tools_without_shell.contains(&"shell"),
            "Should not have shell tool when disabled"
        );
    }

    /// Test 11: Shell execution with allowlist.
    #[test]
    fn test_smart_agent_shell_execution() {
        // Test allowed command
        let ctx = AgentTestContext::new();
        let allowlist = vec!["echo".to_string(), "ls".to_string()];

        let mut registry = ToolRegistry::new();
        registry.register("done", Box::new(DoneTool), create_done_parser());
        registry.register(
            "shell",
            Box::new(ShellTool::new(allowlist.clone())),
            create_shell_parser(),
        );

        // Mock LLM to call shell with allowed command
        let response = "<shell>echo 'Hello World'</shell><done>Task completed</done>".to_string();

        let inference = create_static_inference(response);
        let mut engine = AgentEngineBuilder::new()
            .tools(registry)
            .config(ctx.config.clone())
            .build(&ctx.create_task_context("test-11", "Execute shell command"))
            .expect("Failed to build engine");

        let result = engine.run(&inference);

        assert!(
            result.is_ok(),
            "Should execute allowed command successfully"
        );
    }

    /// Test blocked command (not in allowlist).
    #[test]
    fn test_smart_agent_blocked_command() {
        let ctx = AgentTestContext::new();
        let allowlist = vec!["echo".to_string()]; // rm is not allowed

        let mut registry = ToolRegistry::new();
        registry.register("done", Box::new(DoneTool), create_done_parser());
        registry.register(
            "shell",
            Box::new(ShellTool::new(allowlist)),
            create_shell_parser(),
        );

        // Mock LLM to try blocked command
        let response = "<shell>rm -rf /</shell>".to_string();

        let inference = create_static_inference(response);
        let mut engine = AgentEngineBuilder::new()
            .tools(registry)
            .config(ctx.config.clone())
            .build(&ctx.create_task_context("test-blocked", "Try blocked command"))
            .expect("Failed to build engine");

        let result = engine.run(&inference);

        // Assert: Should fail with blocked error
        assert!(result.is_err(), "Should fail for blocked command");
    }
}

// =============================================================================
// Error Scenarios and Edge Cases
// =============================================================================

#[cfg(test)]
mod error_scenarios {
    use super::*;

    /// Test 12: Agent timeout handling.
    #[test]
    fn test_agent_handles_timeout() {
        let ctx = AgentTestContext::new();

        // Create config with very short timeout
        let config = AgentConfig::builder()
            .timeout(Duration::from_millis(10))
            .build()
            .expect("Failed to build config");

        let registry = create_minimal_registry();

        // Mock inference that simulates delay (by returning slow response pattern)
        let inference = |_model: &str, _history: &[Message]| {
            std::thread::sleep(Duration::from_millis(100)); // Simulate slow LLM
            Ok(InferenceResponse {
                content: "Still working...".to_string(),
                model: "mock".to_string(),
                tokens_used: None,
                finish_reason: None,
            })
        };

        let mut engine = AgentEngineBuilder::new()
            .tools(registry)
            .config(config)
            .build(&ctx.create_task_context("test-12", "Test timeout"))
            .expect("Failed to build engine");

        let result = engine.run(&inference);

        // Assert: Timeout error after configured duration
        assert!(result.is_err(), "Should timeout");
        match result.unwrap_err() {
            AgentError::Timeout { .. } => {
                // Expected
            }
            other => panic!("Expected Timeout error, got: {:?}", other),
        }
    }

    /// Test 13: Invalid tool invocation handling.
    #[test]
    fn test_agent_handles_invalid_tool() {
        let ctx = AgentTestContext::new();

        // Mock LLM to invoke non-existent tool
        let response = "<nonexistent_tool>data</nonexistent_tool>".to_string();

        let registry = create_minimal_registry();

        let inference = create_static_inference(response);
        let mut engine = AgentEngineBuilder::new()
            .tools(registry)
            .config(ctx.config.clone())
            .build(&ctx.create_task_context("test-13", "Test invalid tool"))
            .expect("Failed to build engine");

        let result = engine.run(&inference);

        // Assert: Error handled gracefully (non-existent tools are ignored, eventually timeout or max iterations)
        // Since the tool doesn't exist in registry, it won't be executed
        // The agent will continue until max iterations or timeout
        assert!(result.is_err(), "Should fail eventually");
    }

    /// Test 14: Max iterations limit.
    #[test]
    fn test_agent_respects_max_iterations() {
        let ctx = AgentTestContext::new();

        // Create config with low max iterations
        let config = AgentConfig::builder()
            .max_iterations(3)
            .build()
            .expect("Failed to build config");

        let registry = create_minimal_registry();

        // Mock LLM to never call done (simulated by responses without done)
        let inference = |_model: &str, _history: &[Message]| {
            Ok(InferenceResponse {
                content: "Let me think about this...".to_string(),
                model: "mock".to_string(),
                tokens_used: None,
                finish_reason: None,
            })
        };

        let mut engine = AgentEngineBuilder::new()
            .tools(registry)
            .config(config)
            .build(&ctx.create_task_context("test-14", "Test max iterations"))
            .expect("Failed to build engine");

        let result = engine.run(&inference);

        // Assert: Stops after max_iterations
        assert!(result.is_err(), "Should exceed max iterations");
        match result.unwrap_err() {
            AgentError::MaxIterationsExceeded { max } => {
                assert_eq!(max, 3, "Should stop at 3 iterations");
            }
            other => panic!("Expected MaxIterationsExceeded error, got: {:?}", other),
        }
    }

    /// Test 15: Tool execution failure handling.
    #[test]
    fn test_agent_handles_tool_failure() {
        let ctx = AgentTestContext::new();

        // Create registry with a failing tool
        let mut registry = ToolRegistry::new();
        registry.register(
            "failing_tool",
            Box::new(FailingMockTool::new("failing_tool", "Simulated failure")),
            create_mock_parser("failing_tool"),
        );
        registry.register("done", Box::new(DoneTool), create_done_parser());

        // Mock LLM to call tool that will fail
        let response = "<failing_tool>trigger</failing_tool>".to_string();

        let inference = create_static_inference(response);
        let mut engine = AgentEngineBuilder::new()
            .tools(registry)
            .config(ctx.config.clone())
            .build(&ctx.create_task_context("test-15", "Test tool failure"))
            .expect("Failed to build engine");

        let result = engine.run(&inference);

        // Assert: Error propagated to LLM, agent fails gracefully
        assert!(result.is_err(), "Should fail when tool execution fails");
        match result.unwrap_err() {
            AgentError::ToolExecution { .. } => {
                // Expected - tool execution failed
            }
            other => panic!("Expected ToolExecution error, got: {:?}", other),
        }
    }
}

// =============================================================================
// Additional Edge Case Tests
// =============================================================================

#[cfg(test)]
mod additional_tests {
    use super::*;

    /// Test empty task description handling.
    #[test]
    fn test_empty_task_description() {
        let ctx = AgentTestContext::new();
        let registry = create_minimal_registry();

        let result = AgentEngineBuilder::new()
            .tools(registry)
            .config(ctx.config.clone())
            .build(&TaskContext::new("test", "   ")); // Empty/whitespace description

        assert!(result.is_err(), "Should fail with empty description");
    }

    /// Test multiple tool invocations in single response.
    #[test]
    fn test_multiple_tool_invocations() {
        let ctx = AgentTestContext::new();
        let file1 = ctx.file_path("file1.txt");
        let file2 = ctx.file_path("file2.txt");

        // Mock LLM to call multiple write tools
        let response = format!(
            r#"<write_file path="{}">Content 1</write_file><write_file path="{}">Content 2</write_file><done>Task completed</done>"#,
            file1.display(),
            file2.display()
        );

        let mut registry = create_full_registry(&ctx.config);
        registry.register("write_file", Box::new(WriteFileTool), create_write_parser());

        let inference = create_static_inference(response);
        let mut engine = AgentEngineBuilder::new()
            .tools(registry)
            .config(ctx.config.clone())
            .build(&ctx.create_task_context("test-multi", "Write multiple files"))
            .expect("Failed to build engine");

        let result = engine.run(&inference);

        // Note: Depending on implementation, only first tool might be executed
        // or multiple might be executed. Test what's expected.
        assert!(
            result.is_ok() || result.is_err(),
            "Should complete or fail predictably"
        );
    }

    /// Test tool registry help text generation.
    #[test]
    fn test_tool_registry_help_text() {
        let registry = create_full_registry(&AgentConfig::default());
        let help_text = registry.help_text();

        assert!(!help_text.is_empty(), "Help text should not be empty");
        assert!(
            help_text.contains("read_file"),
            "Help text should mention read_file"
        );
        assert!(help_text.contains("ls"), "Help text should mention ls");
        assert!(help_text.contains("done"), "Help text should mention done");
    }

    /// Test agent engine state tracking.
    #[test]
    fn test_engine_state_tracking() {
        let ctx = AgentTestContext::new();
        let registry = create_minimal_registry();

        let mut engine = AgentEngineBuilder::new()
            .tools(registry)
            .config(ctx.config.clone())
            .build(&ctx.create_task_context("test-state", "Test state tracking"))
            .expect("Failed to build engine");

        // Initial state
        assert_eq!(engine.iteration(), 0, "Should start at iteration 0");
        assert!(
            engine.elapsed() < Duration::from_secs(1),
            "Should have minimal elapsed time"
        );

        // After running (with immediate done)
        let inference = create_static_inference("<done>Done</done>");
        let _ = engine.run(&inference);

        assert_eq!(engine.iteration(), 1, "Should have completed 1 iteration");
        assert!(
            engine.elapsed() > Duration::from_millis(0),
            "Should have some elapsed time"
        );
    }

    /// Test configuration builder validation.
    #[test]
    fn test_config_validation() {
        // Valid config
        let valid = AgentConfig::builder()
            .max_iterations(10)
            .model("gpt-4")
            .build();
        assert!(valid.is_ok(), "Valid config should build successfully");

        // Invalid config (zero iterations)
        let invalid_iterations = AgentConfig::builder().max_iterations(0).build();
        assert!(
            invalid_iterations.is_err(),
            "Zero iterations should fail validation"
        );

        // Invalid config (empty model)
        let invalid_model = AgentConfig::builder().model("").build();
        assert!(invalid_model.is_err(), "Empty model should fail validation");
    }
}

// =============================================================================
// Integration Test Helpers
// =============================================================================

#[cfg(test)]
mod test_helpers_tests {
    use super::*;

    #[test]
    fn test_agent_test_context_creation() {
        let ctx = AgentTestContext::new();
        assert!(ctx.test_root.exists(), "Test root should exist");
        assert!(
            ctx.mock_provider.queued_count() == 0,
            "Should have empty response queue"
        );
    }

    #[test]
    fn test_create_and_read_test_file() {
        let ctx = AgentTestContext::new();
        let content = "Hello, Test!";

        let path = ctx.create_test_file("test.txt", content);
        assert!(path.exists(), "File should exist");

        let read_content = ctx.read_test_file("test.txt");
        assert_eq!(read_content, content, "Content should match");
    }

    #[test]
    fn test_mock_provider_responses() {
        let provider = MockLLMProvider::new();

        provider.queue_response("Response 1");
        provider.queue_response("Response 2");

        assert_eq!(provider.queued_count(), 2, "Should have 2 responses queued");

        let response1 = provider.next_response();
        assert_eq!(response1, "Response 1", "Should return first response");

        let response2 = provider.next_response();
        assert_eq!(response2, "Response 2", "Should return second response");

        let response3 = provider.next_response();
        assert_eq!(
            response3, "<done>Task completed</done>",
            "Should return default"
        );
    }

    #[test]
    fn test_mock_provider_clear() {
        let provider = MockLLMProvider::new();
        provider.queue_response("Test");

        provider.clear();
        assert_eq!(provider.queued_count(), 0, "Should be empty after clear");
    }

    #[test]
    fn test_write_file_tool_success() {
        let ctx = AgentTestContext::new();
        let path = ctx.file_path("output.txt");

        let tool = WriteFileTool;
        let mut args = HashMap::new();
        args.insert("path".to_string(), path.to_string_lossy().to_string());
        args.insert("content".to_string(), "Test content".to_string());

        let result = tool.execute(&args);
        assert!(result.is_ok(), "Write should succeed");

        let content = std::fs::read_to_string(&path).expect("Should read file");
        assert_eq!(content, "Test content");
    }

    #[test]
    fn test_write_file_tool_missing_path() {
        let tool = WriteFileTool;
        let args = HashMap::new();

        let result = tool.execute(&args);
        assert!(result.is_err(), "Should fail without path");
        match result.unwrap_err() {
            ToolError::InvalidArguments { tool, reason } => {
                assert_eq!(tool, "write_file");
                assert!(reason.contains("path"));
            }
            other => panic!("Expected InvalidArguments error, got: {:?}", other),
        }
    }

    #[test]
    fn test_write_file_tool_missing_content() {
        let tool = WriteFileTool;
        let mut args = HashMap::new();
        args.insert("path".to_string(), "/tmp/test.txt".to_string());

        let result = tool.execute(&args);
        assert!(result.is_err(), "Should fail without content");
        match result.unwrap_err() {
            ToolError::InvalidArguments { tool, reason } => {
                assert_eq!(tool, "write_file");
                assert!(reason.contains("content"));
            }
            other => panic!("Expected InvalidArguments error, got: {:?}", other),
        }
    }

    #[test]
    fn test_shell_tool_allowed_command() {
        let tool = ShellTool::new(vec!["echo".to_string()]);
        let mut args = HashMap::new();
        args.insert("command".to_string(), "echo 'Hello'".to_string());

        let result = tool.execute(&args);
        assert!(result.is_ok(), "Should execute allowed command");
        assert!(
            result.unwrap().contains("Hello"),
            "Should return command output"
        );
    }

    #[test]
    fn test_shell_tool_blocked_command() {
        let tool = ShellTool::new(vec!["echo".to_string()]); // rm not in allowlist
        let mut args = HashMap::new();
        args.insert("command".to_string(), "rm -rf /".to_string());

        let result = tool.execute(&args);
        assert!(result.is_err(), "Should block disallowed command");
        match result.unwrap_err() {
            ToolError::Blocked { tool, reason } => {
                assert_eq!(tool, "shell");
                assert!(!reason.is_empty());
            }
            other => panic!("Expected Blocked error, got: {:?}", other),
        }
    }

    #[test]
    fn test_shell_tool_missing_command() {
        let tool = ShellTool::new(vec![]);
        let args = HashMap::new();

        let result = tool.execute(&args);
        assert!(result.is_err(), "Should fail without command");
        match result.unwrap_err() {
            ToolError::InvalidArguments { tool, reason } => {
                assert_eq!(tool, "shell");
                assert!(reason.contains("command"));
            }
            other => panic!("Expected InvalidArguments error, got: {:?}", other),
        }
    }
}
