# Agent SDK API Reference

The `agent-sdk` is a Rust library that provides the framework for building Brio-Kernel agents. It abstracts away the complexity of WASM component development and provides common utilities for agent implementation.

## Table of Contents

1. [Overview](#overview)
2. [Core Types](#core-types)
3. [Configuration](#configuration)
4. [StandardAgent Trait](#standardagent-trait)
5. [Tool System](#tool-system)
6. [Built-in Tools](#built-in-tools)
7. [Engine Components](#engine-components)
8. [Error Handling](#error-handling)
9. [Prompt Builder](#prompt-builder)
10. [Utilities](#utilities)

---

## Overview

The Agent SDK provides a complete framework for building autonomous agents:

- **Module Organization**:
  - `agent`: Agent traits and implementations
  - `config`: Environment-based configuration
  - `engine`: ReAct loop and state management
  - `error`: Structured error hierarchy
  - `prompt`: Dynamic prompt construction
  - `tools`: Tool system with security validation
  - `types`: Core data types

### Getting Started

```rust
use agent_sdk::{
    AgentEngine, AgentConfig, ToolRegistry, TaskContext,
    AgentEngineBuilder, PromptBuilder,
};

// Create a task context
let context = TaskContext::new("task-123", "Write a function")
    .with_files(vec!["input.rs"]);

// Configure the agent
let config = AgentConfig::builder()
    .max_iterations(30)
    .verbose(true)
    .build()?;

// Build and run the agent
let engine = AgentEngineBuilder::new()
    .tools(ToolRegistry::new())
    .config(config)
    .build(&context)?;
```

---

## Core Types

### Message

Represents a message in the conversation history between agents and LLMs.

```rust
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Message {
    pub role: Role,
    pub content: String,
    pub metadata: Option<HashMap<String, String>>,
}
```

**Methods**:
- `Message::new(role: Role, content: impl Into<String>)` - Create a new message
- `Message::system(content)` - Create a system message
- `Message::user(content)` - Create a user message
- `Message::assistant(content)` - Create an assistant message
- `Message::tool(content)` - Create a tool message
- `.with_metadata(key, value)` - Add metadata to the message

### Role

Identifies the role of a message sender.

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    System,     // System instruction or context
    User,       // User input or query
    Assistant,  // Assistant/AI response
    Tool,       // Tool execution result
}
```

### TaskContext

Contains task metadata and parameters for agent execution.

```rust
#[derive(Clone, Debug, Default)]
pub struct TaskContext {
    pub task_id: String,
    pub description: String,
    pub input_files: Vec<String>,
    pub metadata: Option<serde_json::Value>,
}
```

**Methods**:
- `TaskContext::new(task_id, description)` - Create new context
- `.with_files(files)` - Add input files
- `.with_metadata(metadata)` - Add JSON metadata
- `.has_input_files()` - Check if files are provided
- `.file_count()` - Get number of input files

### ToolInvocation

Represents a tool call extracted from an agent's response.

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct ToolInvocation {
    pub name: String,              // Name of the tool to invoke
    pub args: HashMap<String, String>,  // Arguments for the tool
    pub position: usize,           // Position in the response text
}
```

### ToolResult

Contains the outcome of a tool invocation.

```rust
#[derive(Debug)]
pub struct ToolResult {
    pub success: bool,                    // Execution success status
    pub output: String,                   // Output or result
    pub duration: std::time::Duration,    // Execution time
}
```

---

## Configuration

### AgentConfig

Main configuration struct for agents with environment variable support.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    pub max_iterations: u32,        // Default: 20
    pub model: String,              // Default: "best-available"
    pub timeout: Duration,          // Default: 300s (5 minutes)
    pub verbose: bool,              // Default: false
    pub max_file_size: u64,         // Default: 10MB
    pub max_depth: usize,           // Default: 10
    pub shell_allowlist: Vec<String>,
    pub tool_config: ToolConfig,
}
```

**Environment Variables** (prefix: `BRIO_AGENT_*`):
- `BRIO_AGENT_MAX_ITERATIONS` - Maximum iterations
- `BRIO_AGENT_MODEL` - AI model identifier
- `BRIO_AGENT_TIMEOUT_SECONDS` - Timeout in seconds
- `BRIO_AGENT_VERBOSE` - Enable verbose logging (`1` or `true`)
- `BRIO_AGENT_MAX_FILE_SIZE` - Maximum file size in bytes
- `BRIO_AGENT_MAX_DEPTH` - Directory traversal depth limit

**Methods**:
- `AgentConfig::new()` - Create with defaults
- `AgentConfig::from_env()` - Load from environment
- `AgentConfig::builder()` - Get builder pattern interface
- `.validate()` - Validate configuration values

### AgentConfigBuilder

Fluent builder for constructing configurations.

```rust
let config = AgentConfig::builder()
    .max_iterations(30)
    .model("gpt-4")
    .verbose(true)
    .timeout(Duration::from_secs(600))
    .max_file_size(5 * 1024 * 1024)  // 5MB
    .build()?;
```

### ToolConfig

Tool-specific feature flags.

```rust
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ToolConfig {
    pub enable_write: bool,    // Enable write operations
    pub enable_shell: bool,    // Enable shell commands
    pub enable_list: bool,     // Enable directory listing
}
```

---

## StandardAgent Trait

Trait for standard AI-loop agents with standardized execution flow.

```rust
pub trait StandardAgent: Clone + Send + Sync {
    /// Unique name of the agent (required constant)
    const NAME: &'static str;

    /// Builds the system prompt for this agent (required)
    fn build_prompt(
        &self,
        context: &TaskContext,
        tools: &ToolRegistry,
        config: &StandardAgentConfig,
    ) -> String;

    /// Performs inference using the AI interface (required)
    fn perform_inference(
        &self,
        model: &str,
        history: &[Message],
    ) -> Result<InferenceResponse, AgentError>;

    /// Creates tool registry (optional, default provided)
    fn create_tool_registry(&self, _config: &AgentConfig) -> ToolRegistry {
        ToolRegistry::new()
    }
}
```

### Implementation Example

```rust
use agent_sdk::agent::{StandardAgent, StandardAgentConfig};
use agent_sdk::types::{InferenceResponse, Message, TaskContext};
use agent_sdk::tools::ToolRegistry;
use agent_sdk::AgentError;

#[derive(Clone)]
struct MyAgent;

impl StandardAgent for MyAgent {
    const NAME: &'static str = "my-agent";

    fn build_prompt(
        &self,
        context: &TaskContext,
        _tools: &ToolRegistry,
        _config: &StandardAgentConfig,
    ) -> String {
        format!("You are {}. Task: {}", Self::NAME, context.description)
    }

    fn perform_inference(
        &self,
        _model: &str,
        _history: &[Message],
    ) -> Result<InferenceResponse, AgentError> {
        Ok(InferenceResponse {
            content: "Test".to_string(),
            model: "test".to_string(),
            tokens_used: None,
            finish_reason: Some("stop".to_string()),
        })
    }
}
```

### run_standard_agent

Helper function to execute a standard agent:

```rust
use agent_sdk::agent::{run_standard_agent, StandardAgentConfig};

let context = TaskContext::new("task-123", "Write a function");
let config = StandardAgentConfig::default();
let result = run_standard_agent(&MyAgent, &context, &config)?;
```

---

## Tool System

### Tool Trait

Core trait for implementing tools.

```rust
pub trait Tool: Send + Sync {
    fn name(&self) -> Cow<'static, str>;
    fn description(&self) -> Cow<'static, str>;
    fn execute(&self, args: &HashMap<String, String>) -> Result<String, ToolError>;
}
```

### ToolRegistry

Manages tool registration and execution.

```rust
pub struct ToolRegistry {
    tools: HashMap<String, Box<dyn Tool>>,
    parsers: HashMap<String, Arc<ToolParser>>,
}
```

**Methods**:
- `ToolRegistry::new()` - Create empty registry
- `.register(name, tool, parser)` - Register a tool with its parser
- `.available_tools()` - List registered tool names
- `.help_text()` - Get formatted tool descriptions
- `.execute_all(input)` - Parse and execute all tool invocations in input

### ToolParser

Parses tool invocations from agent responses.

```rust
pub struct ToolParser {
    regex: Regex,
    extractor: ArgExtractor,
}

impl ToolParser {
    pub fn new<E>(pattern: &str, extractor: E) -> Result<Self, regex::Error>
    where E: Fn(&Captures) -> HashMap<String, String> + Send + Sync + 'static;
    
    pub fn parse(&self, input: &str) -> Vec<ToolInvocation>;
}
```

### Tool Registration Example

```rust
use agent_sdk::tools::{ToolRegistry, ToolParser};
use std::sync::Arc;
use regex::Captures;

let mut registry = ToolRegistry::new();

let parser = Arc::new(
    ToolParser::new(r"<tool>(.*?)</tool>", |caps: &Captures| {
        let mut args = HashMap::new();
        args.insert("content".to_string(), caps[1].to_string());
        args
    })?
);

registry.register("my_tool", Box::new(MyTool), parser);
```

---

## Built-in Tools

### File System Tools

#### ReadFileTool

Reads file contents with size validation.

```rust
pub struct ReadFileTool {
    max_size: u64,  // Maximum file size in bytes
}

// Usage: <read_file path="path/to/file" />
let tool = ReadFileTool::new(1024 * 1024); // 1MB max
```

#### WriteFileTool

Writes content to files, creating parent directories as needed.

```rust
pub struct WriteFileTool;

// Usage: <write_file path="path/to/file">content</write_file>
let tool = WriteFileTool;
```

#### ListDirectoryTool

Lists directory contents with type indicators.

```rust
pub struct ListDirectoryTool {
    max_depth: usize,  // Reserved for future use
}

// Usage: <ls path="path/to/directory" />
let tool = ListDirectoryTool::new(10);
```

### Shell Tool

#### ShellTool

Executes shell commands with allowlist validation.

```rust
pub struct ShellTool {
    allowlist: Vec<String>,  // Allowed command prefixes
}

// Usage: <shell>command</shell>
let allowlist = vec!["ls".to_string(), "cat".to_string(), "echo".to_string()];
let tool = ShellTool::new(allowlist);
```

**Security**: Commands must be in allowlist; dangerous characters (`;`, `&`, `|`, `>`, `<`, `` ` ``, `$`, `(`) are blocked.

### Search Tool

#### GrepTool

Searches files with regex patterns.

```rust
pub struct GrepTool;

// Usage: <grep pattern="regex" path="file_or_dir" [case_insensitive="true|false"] [max_results="1000"] />
let tool = GrepTool::new();
```

**Features**:
- Single file or directory search
- Case-insensitive option
- Result limit (default 1000)
- DFA size limit for ReDoS protection (10MB)

### Control Tools

#### DoneTool

Marks task as complete.

```rust
pub struct DoneTool;

// Usage: <done>summary of completion</done>
let tool = DoneTool;
```

### Branch Tools

#### CreateBranchTool

Creates isolated workspaces for parallel work.

```rust
pub struct CreateBranchTool {
    creation_callback: BranchCreationCallback,
}

// Usage: <create_branch name="branch-name" [parent="parent-id"] [inherit_config="true"] />
let tool = CreateBranchTool::new(creation_callback);
```

#### ListBranchesTool

Lists all active branches.

```rust
pub struct ListBranchesTool {
    list_callback: BranchListCallback,
}

// Usage: <list_branches />
let tool = ListBranchesTool::new(list_callback);
```

---

## Engine Components

### AgentEngine

Core engine that orchestrates agent execution.

```rust
pub struct AgentEngine {
    state: AgentState,
    tools: ToolRegistry,
    config: AgentConfig,
    start_time: Instant,
}
```

**Methods**:
- `AgentEngine::new(context, tools, config)` - Create new engine
- `.run(inference_fn)` - Run until completion or max iterations
- `.run_with_prompt(system_prompt, inference_fn)` - Run with custom system prompt
- `.iteration()` - Get current iteration count
- `.elapsed()` - Get elapsed time
- `.history()` - Get conversation history

### AgentEngineBuilder

Builder pattern for constructing engines.

```rust
let engine = AgentEngineBuilder::new()
    .tools(registry)
    .config(config)
    .build(&context)?;
```

### AgentState

Internal state management.

```rust
pub(crate) struct AgentState {
    history: Vec<Message>,
    iteration: u32,
}
```

### ReActLoop

The engine implements a ReAct (Reasoning + Acting) loop:

1. **Reason**: Agent analyzes task and plans steps
2. **Act**: Agent calls tools using XML syntax
3. **Observe**: Results are added to conversation history
4. **Repeat**: Loop continues until task complete or limit reached

### InferenceFn

Type alias for inference functions:

```rust
pub type InferenceFn = dyn Fn(&str, &[Message]) -> Result<InferenceResponse, AgentError>;
```

---

## Error Handling

### AgentError

Top-level error enum for agent operations.

```rust
#[derive(Error, Debug)]
pub enum AgentError {
    Inference(InferenceError),
    ToolExecution(ToolError),
    Task(TaskError),
    FileSystem(FileSystemError),
    MaxIterationsExceeded { max: u32 },
    Timeout { elapsed: Duration },
    Context { context: String, source: Box<dyn Error + Send + Sync> },
}
```

### ToolError

Errors during tool execution.

```rust
#[derive(Error, Debug)]
pub enum ToolError {
    NotFound { name: String },
    InvalidArguments { tool: String, reason: String },
    ExecutionFailed { tool: String, source: Box<dyn Error + Send + Sync> },
    Blocked { tool: String, reason: String },
    ResourceLimitExceeded { tool: String, resource: String },
}
```

### FileSystemError

Errors from file system operations.

```rust
#[derive(Error, Debug)]
pub enum FileSystemError {
    PathTraversal { path: PathBuf },
    NotFound { path: PathBuf },
    FileTooLarge { path: PathBuf, size: u64, max_size: u64 },
    PermissionDenied { path: PathBuf },
    Io(std::io::Error),
    InvalidPath(String),
}
```

### InferenceError

Errors from AI inference.

```rust
#[derive(Error, Debug)]
pub enum InferenceError {
    ApiError(String),
    InvalidModel { model: String },
    RateLimited { retry_after: Option<Duration> },
    Network(String),
    ParseError(String),
}
```

### ResultExt Trait

Convenience trait for adding context to errors:

```rust
pub trait ResultExt<T, E> {
    fn with_context<C, F>(self, f: F) -> Result<T, AgentError>
    where F: FnOnce() -> C, C: std::fmt::Display;
}

// Usage
some_result.with_context(|| "Failed to perform operation")?;
```

### Error Propagation

Errors bubble up through the hierarchy:
1. Tool execution errors → `ToolError`
2. File operations → `FileSystemError`
3. Agent-level issues → `AgentError`
4. Context can be added at any level using `with_context`

---

## Prompt Builder

### PromptBuilder

Constructs system prompts programmatically for different agent types.

```rust
pub struct PromptBuilder;

impl PromptBuilder {
    pub fn build_smart_agent(context, tools, config) -> String;
    pub fn build_coder_agent(context, tools, config) -> String;
    pub fn build_reviewer_agent(context, tools, config) -> String;
    pub fn build_council_agent(context, tools, config) -> String;
}
```

### Usage Examples

```rust
use agent_sdk::PromptBuilder;

// Build prompts for different agent types
let smart_prompt = PromptBuilder::build_smart_agent(&context, &tools, &config);
let coder_prompt = PromptBuilder::build_coder_agent(&context, &tools, &config);
let reviewer_prompt = PromptBuilder::build_reviewer_agent(&context, &tools, &config);
let council_prompt = PromptBuilder::build_council_agent(&context, &tools, &config);
```

### Tool Description Formatting

Tools automatically include their descriptions in prompts via `ToolRegistry::help_text()`:

```xml
<read_file path="path/to/file" /> - Read content from a file
<write_file path="path/to/file">content</write_file> - Write content to a file
<ls path="path/to/directory" /> - List directory contents
```

---

## Utilities

### Logging Setup

```rust
// Initialize tracing subscriber
agent_sdk::init_logging()?;
```

### Path Validation

```rust
use agent_sdk::tools::{validate_path, validate_file_size};

// Validate path against directory traversal
let validated = validate_path("../etc/passwd", &base_dir)?;

// Check file size
validate_file_size(&path, 10 * 1024 * 1024)?;
```

### Shell Command Validation

```rust
use agent_sdk::tools::validate_shell_command;

let allowlist = vec!["ls", "cat", "echo"];
validate_shell_command("ls -la", &allowlist)?;  // OK
validate_shell_command("rm -rf /", &allowlist)?; // Error - not in allowlist
validate_shell_command("ls; rm /", &allowlist)?; // Error - dangerous chars
```

### SecureFilePath (Type-State Pattern)

```rust
use agent_sdk::tools::{SecureFilePath, Unvalidated, Validated};

// Create unvalidated path
let unvalidated = SecureFilePath::<Unvalidated>::new("file.txt");

// Validate (transitions to Validated state)
let validated = unvalidated.validate(&base_dir)?;

// Perform operations on validated path
let content = validated.read_with_limit(1024)?;
validated.write("new content")?;
```

### Version

```rust
let version = agent_sdk::VERSION; // e.g., "0.1.0"
```

### Current Directory

```rust
let current_dir = agent_sdk::current_dir()?;
```

---

## Common Patterns

### Custom Tool Implementation

```rust
use agent_sdk::tools::Tool;
use agent_sdk::error::ToolError;
use std::borrow::Cow;
use std::collections::HashMap;

pub struct MyTool;

impl Tool for MyTool {
    fn name(&self) -> Cow<'static, str> {
        Cow::Borrowed("my_tool")
    }

    fn description(&self) -> Cow<'static, str> {
        Cow::Borrowed("<my_tool arg=\"value\" /> - Description")
    }

    fn execute(&self, args: &HashMap<String, String>) -> Result<String, ToolError> {
        let arg = args.get("arg")
            .ok_or_else(|| ToolError::InvalidArguments {
                tool: "my_tool".to_string(),
                reason: "Missing 'arg'".to_string(),
            })?;
        
        Ok(format!("Result: {}", arg))
    }
}
```

### Complete Agent Setup

```rust
use agent_sdk::*;
use agent_sdk::agent::tools::{DoneTool, ReadFileTool, WriteFileTool, ShellTool};

// 1. Create task context
let context = TaskContext::new("task-123", "Implement feature X");

// 2. Configure agent
let config = AgentConfig::builder()
    .max_iterations(30)
    .verbose(true)
    .build()?;

// 3. Create tool registry
let mut registry = ToolRegistry::new();
registry.register("done", Box::new(DoneTool), create_done_parser());
registry.register("read_file", Box::new(ReadFileTool::new(1024*1024)), create_read_parser());
registry.register("write_file", Box::new(WriteFileTool), create_write_parser());

// 4. Build and run
let engine = AgentEngineBuilder::new()
    .tools(registry)
    .config(config)
    .build(&context)?;
```

---

## Security Considerations

- **Path Traversal**: All file operations validate paths to prevent `../` attacks
- **Shell Injection**: Commands are validated against allowlists and dangerous characters are blocked
- **Resource Limits**: File sizes, execution timeouts, and iteration limits are enforced
- **Type-State Pattern**: File operations use compile-time state tracking for validation

---

## Re-exports

The SDK conveniently re-exports commonly used types at the crate root:

```rust
pub use config::{AgentConfig, AgentConfigBuilder, ToolConfig};
pub use engine::{AgentEngine, AgentEngineBuilder, InferenceFn};
pub use error::{AgentError, FileSystemError, InferenceError, ResultExt, TaskError, ToolError};
pub use prompt::PromptBuilder;
pub use tools::{SecureFilePath, Tool, ToolParser, ToolRegistry, Unvalidated, Validated};
pub use types::{ExecutionResult, InferenceResponse, Message, Role, TaskContext, ToolInvocation, ToolResult};
```
