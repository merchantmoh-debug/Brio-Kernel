# Creating Custom Tools

This guide walks you through building custom tools for Brio-Kernel. Tools are WebAssembly components that provide specific capabilities to agents.

## Prerequisites

Before creating a custom tool, ensure you have:

- **Rust** (1.70+) installed
- **wasm32-wasi** target: `rustup target add wasm32-wasi`
- **wasm-tools** for component generation
- Familiarity with Rust and basic WASM concepts

## What is a Tool?

A tool is a WebAssembly component that:

- Implements a focused, single-purpose operation
- Is typically stateless and idempotent
- Receives input via JSON parameters
- Returns results as JSON or error strings
- Can operate with or without session context

**Tool vs Agent:**

| Aspect | Tool | Agent |
|--------|------|-------|
| **Purpose** | Single operation | Complex task execution |
| **State** | Usually stateless | Maintains conversation state |
| **AI** | No AI/model usage | Uses AI for reasoning |
| **Lifecycle** | Execute and return | ReAct loop with multiple steps |
| **Example** | Read file, search text | Code review, task planning |

**When to create a custom tool:**

- You need a specific operation not covered by existing tools
- You want to encapsulate external API calls
- You need domain-specific validation or processing
- You want to expose new capabilities to agents

## Tool Architecture

### Standard Tool Interface

All tools implement the `tool` interface defined in `brio-core/wit/tool.wit`:

```rust
interface tool {
    record tool-info {
        name: string,
        description: string,
        version: string,
        requires-session: bool,
    }

    info: func() -> tool-info;
    
    execute: func(params: string, session-id: option<string>) 
        -> result<string, string>;
}
```

**Key fields:**
- `name`: Unique tool identifier (snake_case)
- `description`: Human-readable purpose
- `version`: Semantic version (e.g., "1.0.0")
- `requires-session`: Whether tool needs VFS access

### Tool Worlds

Brio provides two world types for tools:

#### 1. Standard Tool World

For stateless tools that don't need filesystem access:

```wit
world standard-tool {
    export tool;
}
```

**Use for:** API calls, calculations, data transformation, external integrations

#### 2. Session-Aware Tool World

For tools that need sandboxed filesystem access:

```wit
world session-aware-tool {
    import session-fs;
    import session-fs-ops;
    export tool;
}
```

**Use for:** File operations, project analysis, build tools

### Tool Lifecycle

```
Agent Request
    ↓
Tool Discovery (info())
    ↓
Parameter Validation
    ↓
execute(params, session-id)
    ↓
Operation Execution
    ↓
Result JSON or Error
```

## Project Setup

### 1. Create Tool Component

Create a new Rust crate in the tools directory:

```bash
cd brio-core/components/tools
mkdir my-tool
cd my-tool
cargo init --lib
```

### 2. Configure Cargo.toml

```toml
[package]
name = "my-tool"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]

[dependencies]
wit-bindgen = "0.24"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
```

### 3. Define WIT Interface

Create `brio-core/wit/my-tool.wit`:

```wit
package brio:core;

interface my-tool {
    /// Input parameters for the tool
    record my-input {
        param1: string,
        param2: option<u32>,
    }
    
    /// Output result from the tool
    record my-output {
        result: string,
        count: u32,
    }
    
    /// The main tool function
    do-something: func(input: my-input) -> result<my-output, string>;
}

world my-tool-world {
    export my-tool;
}
```

### 4. Generate Bindings

In `src/lib.rs`:

```rust
#![allow(missing_docs)]

use wit_bindgen::generate;

generate!({
    world: "my-tool-world",
    path: "../../../wit",
    export_macro_name: "export_my_tool",
});

export_my_tool!(MyTool);
```

## Implementing a Stateless Tool

A stateless tool performs operations without accessing the filesystem or maintaining state.

### Step 1: Define Error Types

```rust
#[derive(Debug, Clone)]
pub enum ToolError {
    InvalidInput(String),
    OperationFailed(String),
    Timeout,
}

impl std::fmt::Display for ToolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ToolError::InvalidInput(msg) => write!(f, "Invalid input: {msg}"),
            ToolError::OperationFailed(msg) => write!(f, "Operation failed: {msg}"),
            ToolError::Timeout => write!(f, "Operation timed out"),
        }
    }
}

impl From<ToolError> for String {
    fn from(err: ToolError) -> Self {
        err.to_string()
    }
}
```

### Step 2: Implement Input Validation

```rust
struct MyTool;

impl MyTool {
    fn validate_input(input: &MyInput) -> Result<(), ToolError> {
        if input.param1.is_empty() {
            return Err(ToolError::InvalidInput(
                "param1 cannot be empty".to_string()
            ));
        }
        
        if let Some(val) = input.param2 {
            if val > 10000 {
                return Err(ToolError::InvalidInput(
                    "param2 exceeds maximum (10000)".to_string()
                ));
            }
        }
        
        Ok(())
    }
}
```

### Step 3: Implement the Tool Interface

```rust
impl exports::brio::core::my_tool::Guest for MyTool {
    fn do_something(input: MyInput) -> Result<MyOutput, String> {
        // Validate input
        Self::validate_input(&input)
            .map_err(|e| e.to_string())?;
        
        // Perform operation
        let result = perform_operation(&input.param1, input.param2)
            .map_err(|e| ToolError::OperationFailed(e.to_string()))?;
        
        Ok(MyOutput {
            result: result.text,
            count: result.items.len() as u32,
        })
    }
}
```

## Implementing a Session-Aware Tool

Session-aware tools can access files within a sandboxed session workspace.

### WIT Definition

```wit
package brio:core;

interface session-fs-ops {
    record directory-entry {
        name: string,
        is-directory: bool,
        size: u64,
    }
    
    read-file: func(session-id: string, path: string) -> result<string, string>;
    list-directory: func(session-id: string, path: string) -> result<list<directory-entry>, string>;
}

world session-aware-tool {
    import session-fs-ops;
    export tool;
}
```

### Implementation Example

```rust
generate!({
    world: "session-aware-tool",
    path: "../../../wit",
    export_macro_name: "export_session_tool",
});

export_session_tool!(SessionTool);

struct SessionTool;

impl SessionTool {
    fn analyze_files(session_id: &str, pattern: &str) -> Result<AnalysisResult, String> {
        // List files in session workspace
        let entries = imports::session_fs_ops::list_directory(session_id, ".")
            .map_err(|e| format!("Failed to list directory: {e}"))?;
        
        let mut matches = Vec::new();
        
        for entry in entries {
            if !entry.is_directory && entry.name.ends_with(".rs") {
                // Read file content
                let content = imports::session_fs_ops::read_file(
                    session_id, 
                    &entry.name
                ).map_err(|e| format!("Failed to read {}: {e}", entry.name))?;
                
                if content.contains(pattern) {
                    matches.push(entry.name);
                }
            }
        }
        
        Ok(AnalysisResult { matches })
    }
}

impl exports::brio::core::tool::Guest for SessionTool {
    fn info() -> exports::brio::core::tool::ToolInfo {
        exports::brio::core::tool::ToolInfo {
            name: "analyze-code".to_string(),
            description: "Analyzes code files in the session workspace".to_string(),
            version: "1.0.0".to_string(),
            requires_session: true,
        }
    }
    
    fn execute(params: String, session_id: Option<String>) -> Result<String, String> {
        let session_id = session_id.ok_or("Session ID required")?;
        
        let input: AnalysisInput = serde_json::from_str(&params)
            .map_err(|e| format!("Invalid JSON: {e}"))?;
        
        let result = Self::analyze_files(&session_id, &input.pattern)?;
        
        serde_json::to_string(&result)
            .map_err(|e| format!("Serialization failed: {e}"))
    }
}
```

## Security Considerations

### Input Validation

Always validate inputs before processing:

```rust
fn validate_path(path: &str) -> Result<(), ToolError> {
    // Reject null bytes
    if path.contains('\0') {
        return Err(ToolError::InvalidInput("Null bytes not allowed".to_string()));
    }
    
    // Prevent path traversal
    if path.contains("../") || path.contains("..\\") {
        return Err(ToolError::InvalidInput("Path traversal not allowed".to_string()));
    }
    
    // Reject absolute paths
    if path.starts_with('/') || path.starts_with("\\") {
        return Err(ToolError::InvalidInput("Absolute paths not allowed".to_string()));
    }
    
    Ok(())
}
```

### Resource Limits

Set limits to prevent abuse:

```rust
const MAX_FILE_SIZE: u64 = 10 * 1024 * 1024; // 10MB
const MAX_LINES: u32 = 10_000;
const MAX_PATTERN_LENGTH: usize = 1000;

fn check_limits(content: &str) -> Result<(), ToolError> {
    if content.len() as u64 > MAX_FILE_SIZE {
        return Err(ToolError::InvalidInput("File too large".to_string()));
    }
    Ok(())
}
```

### Dangerous Operation Prevention

```rust
// Validate shell commands (if tool executes commands)
fn validate_command(cmd: &str) -> Result<(), ToolError> {
    let forbidden = ["rm -rf /", ":(){ :|:& };:", "dd if=/dev/zero"];
    
    for pattern in &forbidden {
        if cmd.contains(pattern) {
            return Err(ToolError::InvalidInput(
                "Dangerous command detected".to_string()
            ));
        }
    }
    
    Ok(())
}
```

## Testing Tools

### Unit Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_path_safe() {
        assert!(MyTool::validate_path("file.txt").is_ok());
        assert!(MyTool::validate_path("src/main.rs").is_ok());
    }

    #[test]
    fn test_validate_path_traversal() {
        assert!(MyTool::validate_path("../etc/passwd").is_err());
        assert!(MyTool::validate_path("foo/../../bar").is_err());
    }

    #[test]
    fn test_tool_execution() {
        let input = MyInput {
            param1: "test".to_string(),
            param2: Some(42),
        };
        
        let result = MyTool::do_something(input);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().count, 1);
    }
}
```

### Integration Testing

For session-aware tools, mock the host interfaces:

```rust
#[test]
fn test_with_mock_session() {
    // This requires a test harness that mocks imports::session_fs_ops
    let tool = SessionTool;
    let params = r#"{"pattern": "fn main"}"#;
    
    // Mock would provide test filesystem
    let result = tool.execute(params.to_string(), Some("test-session".to_string()));
    assert!(result.is_ok());
}
```

## Registration and Usage

### Building the WASM Component

```bash
# Build the tool
cargo build --target wasm32-wasi --release

# Convert to component
wasm-tools component new \
    target/wasm32-wasi/release/my_tool.wasm \
    -o my-tool.wasm

# Validate
wasm-tools validate my-tool.wasm
```

### Registering with the Kernel

Add to supervisor configuration:

```toml
[[tools]]
name = "my-tool"
component_path = "./tools/my-tool.wasm"
description = "My custom tool for specific operations"
```

Or register programmatically:

```rust
use brio_core::tool::ToolRegistry;

let mut registry = ToolRegistry::new();
registry.register_from_wasm("my-tool", "./tools/my-tool.wasm")?;
```

### Making Tools Available to Agents

Update agent tool registry:

```rust
fn create_tool_registry(&self, config: &AgentConfig) -> ToolRegistry {
    let mut registry = ToolRegistry::new();
    
    // Register built-in tools
    registry.register("read_file", Box::new(ReadFileTool), create_read_parser());
    
    // Register custom tool
    registry.register("my_tool", Box::new(MyCustomTool), create_my_tool_parser());
    
    registry
}
```

## Example: Weather Lookup Tool

Complete example of a stateless tool that fetches weather data:

### WIT Definition

```wit
package brio:core;

interface weather-tool {
    record weather-input {
        city: string,
        units: option<string>,
    }
    
    record weather-output {
        temperature: f32,
        humidity: u32,
        description: string,
    }
    
    get-weather: func(input: weather-input) -> result<weather-output, string>;
}

world weather-tool-world {
    export weather-tool;
}
```

### Implementation

```rust
#![allow(missing_docs)]

use serde::{Deserialize, Serialize};
use wit_bindgen::generate;

generate!({
    world: "weather-tool-world",
    path: "../../../wit",
    export_macro_name: "export_weather_tool",
});

export_weather_tool!(WeatherTool);

const MAX_CITY_LENGTH: usize = 100;

#[derive(Debug, Clone)]
pub enum WeatherError {
    InvalidCity(String),
    ApiError(String),
    NetworkError(String),
}

impl std::fmt::Display for WeatherError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WeatherError::InvalidCity(msg) => write!(f, "Invalid city: {msg}"),
            WeatherError::ApiError(msg) => write!(f, "API error: {msg}"),
            WeatherError::NetworkError(msg) => write!(f, "Network error: {msg}"),
        }
    }
}

impl From<WeatherError> for String {
    fn from(err: WeatherError) -> Self {
        err.to_string()
    }
}

#[derive(Debug, Deserialize)]
struct ApiResponse {
    main: MainData,
    weather: Vec<WeatherData>,
}

#[derive(Debug, Deserialize)]
struct MainData {
    temp: f32,
    humidity: u32,
}

#[derive(Debug, Deserialize)]
struct WeatherData {
    description: String,
}

struct WeatherTool;

impl WeatherTool {
    fn validate_city(city: &str) -> Result<(), WeatherError> {
        if city.is_empty() {
            return Err(WeatherError::InvalidCity("City cannot be empty".to_string()));
        }
        
        if city.len() > MAX_CITY_LENGTH {
            return Err(WeatherError::InvalidCity(
                format!("City name too long (max {MAX_CITY_LENGTH})")
            ));
        }
        
        // Reject potentially dangerous characters
        if city.contains('\0') || city.contains('\n') {
            return Err(WeatherError::InvalidCity("Invalid characters in city name".to_string()));
        }
        
        Ok(())
    }
    
    fn fetch_weather(city: &str, units: &str) -> Result<ApiResponse, WeatherError> {
        // In production, make actual HTTP request
        // This is a simplified example
        
        // Validate city
        Self::validate_city(city)?;
        
        // Mock implementation for demonstration
        Ok(ApiResponse {
            main: MainData {
                temp: if units == "imperial" { 72.0 } else { 22.0 },
                humidity: 65,
            },
            weather: vec![WeatherData {
                description: "partly cloudy".to_string(),
            }],
        })
    }
}

impl exports::brio::core::weather_tool::Guest for WeatherTool {
    fn get_weather(
        input: exports::brio::core::weather_tool::WeatherInput
    ) -> Result<exports::brio::core::weather_tool::WeatherOutput, String> {
        let units = input.units.as_deref().unwrap_or("metric");
        
        let api_response = Self::fetch_weather(&input.city, units)
            .map_err(|e| e.to_string())?;
        
        let description = api_response.weather
            .first()
            .map(|w| w.description.clone())
            .unwrap_or_else(|| "unknown".to_string());
        
        Ok(exports::brio::core::weather_tool::WeatherOutput {
            temperature: api_response.main.temp,
            humidity: api_response.main.humidity,
            description,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_city_valid() {
        assert!(WeatherTool::validate_city("London").is_ok());
        assert!(WeatherTool::validate_city("New York").is_ok());
    }

    #[test]
    fn test_validate_city_empty() {
        assert!(WeatherTool::validate_city("").is_err());
    }

    #[test]
    fn test_validate_city_too_long() {
        let long_city = "a".repeat(101);
        assert!(WeatherTool::validate_city(&long_city).is_err());
    }

    #[test]
    fn test_validate_city_invalid_chars() {
        assert!(WeatherTool::validate_city("City\nName").is_err());
        assert!(WeatherTool::validate_city("City\0Name").is_err());
    }
}
```

## Best Practices

### Naming Conventions

- **Tool names**: Use `snake_case` (e.g., `read_file`, `weather_lookup`)
- **Interface names**: Use `kebab-case` in WIT (e.g., `read-file-tool`)
- **Struct names**: Use `PascalCase` (e.g., `ReadFileTool`)
- **Error variants**: Use `PascalCase` with descriptive names

### Error Message Clarity

```rust
// Good: Specific and actionable
Err("File not found: src/main.rs. Check the path and try again.".to_string())

// Bad: Vague and unhelpful
Err("Error".to_string())
```

### Performance Considerations

- **Lazy loading**: Load large resources only when needed
- **Caching**: Cache expensive operations when appropriate
- **Streaming**: For large data, consider streaming instead of loading all at once
- **Timeouts**: Set reasonable timeouts for network operations

```rust
const OPERATION_TIMEOUT: Duration = Duration::from_secs(30);

fn with_timeout<F, T>(operation: F) -> Result<T, ToolError>
where
    F: FnOnce() -> Result<T, ToolError>,
{
    // Implementation with timeout
}
```

### Documentation

```rust
//! Weather Lookup Tool
//!
//! Provides weather information for cities worldwide.
//! 
//! # Example
//! 
//! ```
//! let input = WeatherInput {
//!     city: "London".to_string(),
//!     units: Some("metric".to_string()),
//! };
//! let result = WeatherTool::get_weather(input)?;
//! ```
//! 
//! # Security
//! 
//! - City names are validated for length and content
//! - No external API keys are exposed in errors
```

### Tool Registry Checklist

Before deploying a tool:

- [ ] Input validation covers all edge cases
- [ ] Error messages are clear and helpful
- [ ] Resource limits are enforced
- [ ] Security validations are in place
- [ ] Unit tests cover main scenarios
- [ ] Documentation is complete
- [ ] WIT interface is properly defined
- [ ] Tool works in both standalone and agent contexts

## Resources

- [Tool SDK Reference](../api-reference/tool-sdk.md)
- [Standard Tools](../concepts/tools.md)
- [Agents Guide](./creating-agents.md)
- [Example: Read File Tool](../../brio-core/components/tools/tool_read_file/src/lib.rs)
- [Example: Grep Tool](../../brio-core/components/tools/tool_grep/src/lib.rs)

---

Creating custom tools extends Brio-Kernel's capabilities while maintaining security and performance. Start with simple stateless tools and progress to session-aware tools as needed.
