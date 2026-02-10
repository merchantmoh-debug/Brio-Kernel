//! WASM Tool Bridge - Integration layer between WASM components and agent-sdk `ToolRegistry`.
//!
//! This module provides a bridge that allows WASM component tools to be registered
//! with the agent-sdk's `ToolRegistry`. It handles:
//! - Wrapping WASM components in the `Tool` trait
//! - Converting XML-style parameters (`HashMap`) to JSON for WASM tools
//! - Abstracting WIT interface calls
//! - Factory functions for easy tool creation
//!
//! # Architecture
//!
//! The bridge uses a layered architecture:
//! 1. **`WitInterface` trait** - Abstraction over WIT calls (allows mocking/testing)
//! 2. **`WasmToolBridge`** - Implements `Tool` trait, wraps WASM components
//! 3. **Parameter converters** - Convert `HashMap` to tool-specific JSON formats
//! 4. **Factory functions** - Convenient constructors for each tool type
//!
//! # Example Usage
//!
//! ```ignore
//! use agent_sdk::tools::wasm_bridge::{WasmToolBridge, MockWitInterface};
//! use agent_sdk::ToolRegistry;
//!
//! let mut registry = ToolRegistry::new();
//!
//! // Create a bridged shell tool with mock interface
//! let shell_tool = WasmToolBridge::shell_tool(Box::new(MockWitInterface::new()));
//! registry.register("shell", Box::new(shell_tool), create_shell_parser());
//! ```

use crate::error::ToolError;
use crate::tools::Tool;
use serde_json::Value;
use std::borrow::Cow;
use std::collections::HashMap;

/// Abstraction over WIT interface calls.
///
/// This trait abstracts the actual WASM runtime (e.g., wasmtime) to allow:
/// - Testing with mock implementations
/// - Swapping WASM runtimes without changing bridge code
/// - Proper separation of concerns per dependency inversion principle
///
/// # Example
///
/// ```ignore
/// pub struct WasmtimeInterface {
///     // wasmtime engine, store, instance
/// }
///
/// impl WitInterface for WasmtimeInterface {
///     fn call(&self, function: &str, params: &str) -> Result<String, String> {
///         // Actual wasmtime call implementation
///     }
/// }
/// ```
pub trait WitInterface: Send + Sync {
    /// Calls a WASM component function with JSON parameters.
    ///
    /// # Arguments
    ///
    /// * `function` - The name of the function to call (e.g., "execute", "grep", "`read_file`")
    /// * `params` - JSON string containing the function parameters
    ///
    /// # Returns
    ///
    /// - `Ok(String)` - JSON string result on success
    /// - `Err(String)` - Error message on failure
    fn call(&self, function: &str, params: &str) -> Result<String, String>;
}

/// Bridge that wraps a WASM component in the `Tool` trait.
///
/// This struct implements the `Tool` trait, allowing WASM components to be
/// registered with the `ToolRegistry`. It handles parameter conversion and
/// delegates actual execution to the WIT interface.
///
/// # Type Parameters
///
/// Uses dynamic dispatch (`Box<dyn WitInterface>`) for flexibility while
/// maintaining Send + Sync bounds required by the Tool trait.
///
/// # Example
///
/// ```ignore
/// let bridge = WasmToolBridge::new(
///     "shell".to_string(),
///     "<shell>command</shell>".to_string(),
///     Box::new(my_wit_interface),
///     ParameterFormat::Array,
/// );
/// ```
pub struct WasmToolBridge {
    name: String,
    description: String,
    wit_interface: Box<dyn WitInterface>,
    param_format: ParameterFormat,
    function_name: String,
}

/// Parameter format for converting `HashMap` to JSON.
///
/// Different WASM tools expect different JSON formats:
/// - `Array`: JSON array for shell tool (e.g., `["ls", "-la"]`)
/// - `Object`: JSON object for most tools (e.g., `{"path": "file.txt"}`)
/// - `GrepArgs`: Special handling for grep tool (pattern + path)
/// - `ReadFileArgs`: Special handling for `read_file` with optional range
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParameterFormat {
    /// JSON array format (used by shell tool)
    Array,
    /// JSON object format (generic)
    Object,
    /// Grep-specific format with pattern and path
    GrepArgs,
    /// Read file with optional line range
    ReadFileArgs,
}

impl WasmToolBridge {
    /// Creates a new WASM tool bridge.
    ///
    /// # Arguments
    ///
    /// * `name` - Tool name (must match registered name in `ToolRegistry`)
    /// * `description` - Tool description in XML format for agent prompts
    /// * `wit_interface` - Implementation of `WitInterface` for WASM calls
    /// * `param_format` - Format for converting `HashMap` args to JSON
    /// * `function_name` - Name of the WIT function to call (e.g., "execute", "grep")
    ///
    /// # Example
    ///
    /// ```ignore
    /// let bridge = WasmToolBridge::new(
    ///     "shell".to_string(),
    ///     "<shell>command</shell>".to_string(),
    ///     wit_interface,
    ///     ParameterFormat::Array,
    ///     "execute".to_string(),
    /// );
    /// ```
    #[must_use]
    pub fn new(
        name: String,
        description: String,
        wit_interface: Box<dyn WitInterface>,
        param_format: ParameterFormat,
        function_name: String,
    ) -> Self {
        Self {
            name,
            description,
            wit_interface,
            param_format,
            function_name,
        }
    }

    /// Creates a shell tool bridge.
    ///
    /// Shell tool expects JSON array format: `["command", "arg1", "arg2"]`
    ///
    /// # Arguments
    ///
    /// * `wit_interface` - Implementation of `WitInterface`
    ///
    /// # Example
    ///
    /// ```ignore
    /// let shell_tool = WasmToolBridge::shell_tool(Box::new(wasmtime_interface));
    /// ```
    #[must_use]
    pub fn shell_tool(wit_interface: Box<dyn WitInterface>) -> Self {
        Self::new(
            "shell".to_string(),
            "<shell>command</shell> - Execute a shell command".to_string(),
            wit_interface,
            ParameterFormat::Array,
            "execute".to_string(),
        )
    }

    /// Creates a grep tool bridge.
    ///
    /// Grep tool expects pattern and path parameters.
    ///
    /// # Arguments
    ///
    /// * `wit_interface` - Implementation of `WitInterface`
    ///
    /// # Example
    ///
    /// ```ignore
    /// let grep_tool = WasmToolBridge::grep_tool(Box::new(wasmtime_interface));
    /// ```
    #[must_use]
    pub fn grep_tool(wit_interface: Box<dyn WitInterface>) -> Self {
        Self::new(
            "grep".to_string(),
            r#"<grep pattern="search" path="file.txt" /> - Search for pattern in file"#.to_string(),
            wit_interface,
            ParameterFormat::GrepArgs,
            "grep".to_string(),
        )
    }

    /// Creates a `read_file` tool bridge.
    ///
    /// Read file tool expects path and optional line range.
    ///
    /// # Arguments
    ///
    /// * `wit_interface` - Implementation of `WitInterface`
    ///
    /// # Example
    ///
    /// ```ignore
    /// let read_tool = WasmToolBridge::read_file_tool(Box::new(wasmtime_interface));
    /// ```
    #[must_use]
    pub fn read_file_tool(wit_interface: Box<dyn WitInterface>) -> Self {
        Self::new(
            "read_file".to_string(),
            r#"<read_file path="path/to/file" [offset="0"] [limit="100"] /> - Read file contents"#
                .to_string(),
            wit_interface,
            ParameterFormat::ReadFileArgs,
            "read_file".to_string(),
        )
    }

    /// Converts `HashMap` arguments to JSON based on parameter format.
    ///
    /// # Errors
    ///
    /// Returns `ToolError::InvalidArguments` if required parameters are missing
    /// or cannot be converted to the expected format.
    fn convert_params(&self, args: &HashMap<String, String>) -> Result<String, ToolError> {
        match self.param_format {
            ParameterFormat::Array => Self::convert_to_array(args),
            ParameterFormat::Object => Self::convert_to_object(args),
            ParameterFormat::GrepArgs => Self::convert_grep_args(args),
            ParameterFormat::ReadFileArgs => Self::convert_read_file_args(args),
        }
    }

    /// Converts arguments to JSON array format.
    ///
    /// Expected input: `HashMap` with "command" key containing full command string.
    /// Output: JSON array split by whitespace (e.g., `["ls", "-la"]`)
    ///
    /// # Errors
    ///
    /// Returns error if "command" parameter is missing.
    fn convert_to_array(args: &HashMap<String, String>) -> Result<String, ToolError> {
        let command = args
            .get("command")
            .ok_or_else(|| ToolError::InvalidArguments {
                tool: "shell".to_string(),
                reason: "Missing 'command' argument".to_string(),
            })?;

        // Split command by whitespace, respecting quotes
        let parts = Self::split_command(command);
        serde_json::to_string(&parts).map_err(|e| ToolError::ExecutionFailed {
            tool: "shell".to_string(),
            source: Box::new(e),
        })
    }

    /// Converts arguments to JSON object format.
    ///
    /// Simply converts the `HashMap` to a JSON object.
    fn convert_to_object(args: &HashMap<String, String>) -> Result<String, ToolError> {
        serde_json::to_string(args).map_err(|e| ToolError::ExecutionFailed {
            tool: "generic".to_string(),
            source: Box::new(e),
        })
    }

    /// Converts arguments for grep tool.
    ///
    /// Expected: "pattern" and "path" parameters.
    /// Output: JSON object with pattern and path.
    ///
    /// # Errors
    ///
    /// Returns error if "pattern" or "path" is missing.
    fn convert_grep_args(args: &HashMap<String, String>) -> Result<String, ToolError> {
        let pattern = args
            .get("pattern")
            .ok_or_else(|| ToolError::InvalidArguments {
                tool: "grep".to_string(),
                reason: "Missing 'pattern' argument".to_string(),
            })?;

        let path = args
            .get("path")
            .ok_or_else(|| ToolError::InvalidArguments {
                tool: "grep".to_string(),
                reason: "Missing 'path' argument".to_string(),
            })?;

        let json_obj = serde_json::json!({
            "pattern": pattern,
            "path": path
        });

        serde_json::to_string(&json_obj).map_err(|e| ToolError::ExecutionFailed {
            tool: "grep".to_string(),
            source: Box::new(e),
        })
    }

    /// Converts arguments for `read_file` tool.
    ///
    /// Expected: "path" parameter, optional "offset" and "limit".
    /// Output: JSON object with path and optional range.
    ///
    /// # Errors
    ///
    /// Returns error if "path" is missing.
    fn convert_read_file_args(args: &HashMap<String, String>) -> Result<String, ToolError> {
        let path = args
            .get("path")
            .ok_or_else(|| ToolError::InvalidArguments {
                tool: "read_file".to_string(),
                reason: "Missing 'path' argument".to_string(),
            })?;

        let mut json_obj = serde_json::json!({
            "path": path
        });

        // Handle optional line range
        if let Some(offset_str) = args.get("offset")
            && let Ok(offset) = offset_str.parse::<u32>()
        {
            json_obj["offset"] = Value::from(offset);
        }

        if let Some(limit_str) = args.get("limit")
            && let Ok(limit) = limit_str.parse::<u32>()
        {
            json_obj["limit"] = Value::from(limit);
        }

        serde_json::to_string(&json_obj).map_err(|e| ToolError::ExecutionFailed {
            tool: "read_file".to_string(),
            source: Box::new(e),
        })
    }

    /// Splits a command string into parts, respecting quoted strings.
    ///
    /// This handles both single and double quotes.
    fn split_command(command: &str) -> Vec<String> {
        let mut parts = Vec::new();
        let mut current = String::new();
        let mut in_quotes = false;
        let mut quote_char = '"';

        for ch in command.chars() {
            if ch == '"' || ch == '\'' {
                if in_quotes && ch == quote_char {
                    in_quotes = false;
                } else if !in_quotes {
                    in_quotes = true;
                    quote_char = ch;
                } else {
                    current.push(ch);
                }
            } else if ch.is_whitespace() && !in_quotes {
                if !current.is_empty() {
                    parts.push(current.clone());
                    current.clear();
                }
            } else {
                current.push(ch);
            }
        }

        if !current.is_empty() {
            parts.push(current);
        }

        parts
    }
}

impl Tool for WasmToolBridge {
    fn name(&self) -> Cow<'static, str> {
        Cow::Owned(self.name.clone())
    }

    fn description(&self) -> Cow<'static, str> {
        Cow::Owned(self.description.clone())
    }

    fn execute(&self, args: &HashMap<String, String>) -> Result<String, ToolError> {
        // Convert HashMap arguments to JSON based on parameter format
        let json_params = self.convert_params(args)?;

        // Call the WASM component via WIT interface
        self.wit_interface
            .call(&self.function_name, &json_params)
            .map_err(|err_msg| ToolError::ExecutionFailed {
                tool: self.name.clone(),
                source: Box::new(std::io::Error::other(err_msg)),
            })
    }
}

/// Mock implementation of `WitInterface` for testing.
///
/// This implementation records all calls and can return predefined responses.
/// Useful for unit testing the bridge without loading actual WASM components.
///
/// # Example
///
/// ```
/// use agent_sdk::tools::wasm_bridge::MockWitInterface;
///
/// let mut mock = MockWitInterface::new();
/// mock.set_response("execute", Ok("output".to_string()));
/// ```
#[derive(Debug, Default)]
pub struct MockWitInterface {
    responses: HashMap<String, Result<String, String>>,
    calls: Vec<(String, String)>, // Records (function, params) for verification
}

impl MockWitInterface {
    /// Creates a new mock interface with no predefined responses.
    #[must_use]
    pub fn new() -> Self {
        Self {
            responses: HashMap::new(),
            calls: Vec::new(),
        }
    }

    /// Sets a response for a specific function.
    ///
    /// # Arguments
    ///
    /// * `function` - The function name to set response for
    /// * `response` - The result to return when this function is called
    pub fn set_response(&mut self, function: &str, response: Result<String, String>) {
        self.responses.insert(function.to_string(), response);
    }

    /// Returns all recorded calls for verification.
    #[must_use]
    pub fn get_calls(&self) -> &[(String, String)] {
        &self.calls
    }

    /// Clears all recorded calls.
    pub fn clear_calls(&mut self) {
        self.calls.clear();
    }
}

impl WitInterface for MockWitInterface {
    fn call(&self, function: &str, _params: &str) -> Result<String, String> {
        // Note: This requires interior mutability, so in practice MockWitInterface
        // would use RefCell or similar. For simplicity in this implementation,
        // we won't record calls in the trait implementation.
        self.responses
            .get(function)
            .cloned()
            .unwrap_or_else(|| Err(format!("No mock response set for function: {function}")))
    }
}

/// Thread-safe mock implementation using interior mutability.
///
/// This version uses `parking_lot::Mutex` for thread-safe call recording.
#[derive(Debug)]
pub struct ThreadSafeMockWitInterface {
    responses: parking_lot::Mutex<HashMap<String, Result<String, String>>>,
    calls: parking_lot::Mutex<Vec<(String, String)>>,
}

impl ThreadSafeMockWitInterface {
    /// Creates a new thread-safe mock interface.
    #[must_use]
    pub fn new() -> Self {
        Self {
            responses: parking_lot::Mutex::new(HashMap::new()),
            calls: parking_lot::Mutex::new(Vec::new()),
        }
    }

    /// Sets a response for a specific function.
    pub fn set_response(&self, function: &str, response: Result<String, String>) {
        self.responses.lock().insert(function.to_string(), response);
    }

    /// Returns all recorded calls for verification.
    #[must_use]
    pub fn get_calls(&self) -> Vec<(String, String)> {
        self.calls.lock().clone()
    }

    /// Clears all recorded calls.
    pub fn clear_calls(&self) {
        self.calls.lock().clear();
    }
}

impl Default for ThreadSafeMockWitInterface {
    fn default() -> Self {
        Self::new()
    }
}

impl WitInterface for ThreadSafeMockWitInterface {
    fn call(&self, function: &str, params: &str) -> Result<String, String> {
        self.calls
            .lock()
            .push((function.to_string(), params.to_string()));

        self.responses
            .lock()
            .get(function)
            .cloned()
            .unwrap_or_else(|| Err(format!("No mock response set for function: {function}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shell_command_splitting() {
        let cases = vec![
            ("ls -la", vec!["ls", "-la"]),
            ("echo hello world", vec!["echo", "hello", "world"]),
            ("echo 'hello world'", vec!["echo", "hello world"]),
            ("echo \"hello world\"", vec!["echo", "hello world"]),
            ("cat file.txt", vec!["cat", "file.txt"]),
        ];

        for (input, expected) in cases {
            let result = WasmToolBridge::split_command(input);
            assert_eq!(
                result,
                expected.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
                "Failed for input: {}",
                input
            );
        }
    }

    #[test]
    fn test_convert_to_array() {
        let mut args = HashMap::new();
        args.insert("command".to_string(), "ls -la".to_string());

        let result = WasmToolBridge::convert_to_array(&args);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), r#"["ls","-la"]"#);
    }

    #[test]
    fn test_convert_to_array_missing_command() {
        let args = HashMap::new();

        let result = WasmToolBridge::convert_to_array(&args);
        assert!(matches!(
            result,
            Err(ToolError::InvalidArguments { tool, reason })
            if tool == "shell" && reason.contains("command")
        ));
    }

    #[test]
    fn test_convert_grep_args() {
        let mut args = HashMap::new();
        args.insert("pattern".to_string(), "test".to_string());
        args.insert("path".to_string(), "file.txt".to_string());

        let result = WasmToolBridge::convert_grep_args(&args);
        assert!(result.is_ok());
        let json_str = result.unwrap();
        assert!(json_str.contains("test"));
        assert!(json_str.contains("file.txt"));
    }

    #[test]
    fn test_convert_grep_args_missing_pattern() {
        let mut args = HashMap::new();
        args.insert("path".to_string(), "file.txt".to_string());

        let result = WasmToolBridge::convert_grep_args(&args);
        assert!(matches!(
            result,
            Err(ToolError::InvalidArguments { tool, reason })
            if tool == "grep" && reason.contains("pattern")
        ));
    }

    #[test]
    fn test_convert_read_file_args() {
        let mut args = HashMap::new();
        args.insert("path".to_string(), "test.txt".to_string());

        let result = WasmToolBridge::convert_read_file_args(&args);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), r#"{"path":"test.txt"}"#);
    }

    #[test]
    fn test_convert_read_file_args_with_range() {
        let mut args = HashMap::new();
        args.insert("path".to_string(), "test.txt".to_string());
        args.insert("offset".to_string(), "10".to_string());
        args.insert("limit".to_string(), "50".to_string());

        let result = WasmToolBridge::convert_read_file_args(&args);
        assert!(result.is_ok());
        let json_str = result.unwrap();
        assert!(json_str.contains("test.txt"));
        assert!(json_str.contains("10"));
        assert!(json_str.contains("50"));
    }

    #[test]
    fn test_wasm_tool_bridge_shell_tool() {
        let mock = ThreadSafeMockWitInterface::new();
        mock.set_response("execute", Ok("file1.txt\nfile2.txt".to_string()));

        let tool = WasmToolBridge::shell_tool(Box::new(mock));

        assert_eq!(tool.name(), "shell");
        assert!(tool.description().contains("shell"));
    }

    #[test]
    fn test_wasm_tool_bridge_execute_success() {
        let mock = ThreadSafeMockWitInterface::new();
        mock.set_response("execute", Ok("command output".to_string()));

        let tool = WasmToolBridge::shell_tool(Box::new(mock));

        let mut args = HashMap::new();
        args.insert("command".to_string(), "ls -la".to_string());

        let result = tool.execute(&args);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "command output");
    }

    #[test]
    fn test_wasm_tool_bridge_execute_failure() {
        let mock = ThreadSafeMockWitInterface::new();
        mock.set_response("execute", Err("Command not found".to_string()));

        let tool = WasmToolBridge::shell_tool(Box::new(mock));

        let mut args = HashMap::new();
        args.insert("command".to_string(), "unknown_cmd".to_string());

        let result = tool.execute(&args);
        assert!(result.is_err());
    }

    #[test]
    fn test_grep_tool_bridge() {
        let mock = ThreadSafeMockWitInterface::new();
        let tool = WasmToolBridge::grep_tool(Box::new(mock));

        assert_eq!(tool.name(), "grep");
        assert!(tool.description().contains("grep"));
    }

    #[test]
    fn test_read_file_tool_bridge() {
        let mock = ThreadSafeMockWitInterface::new();
        let tool = WasmToolBridge::read_file_tool(Box::new(mock));

        assert_eq!(tool.name(), "read_file");
        assert!(tool.description().contains("read_file"));
    }

    #[test]
    fn test_mock_wit_interface() {
        let mock = ThreadSafeMockWitInterface::new();
        mock.set_response("test_func", Ok("success".to_string()));

        let result = mock.call("test_func", "{}");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "success");

        // Check call was recorded
        let calls = mock.get_calls();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, "test_func");
        assert_eq!(calls[0].1, "{}");
    }

    #[test]
    fn test_mock_wit_interface_no_response() {
        let mock = ThreadSafeMockWitInterface::new();

        let result = mock.call("unknown_func", "{}");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("No mock response"));
    }

    #[test]
    fn test_convert_to_object() {
        let mut args = HashMap::new();
        args.insert("key1".to_string(), "value1".to_string());
        args.insert("key2".to_string(), "value2".to_string());

        let result = WasmToolBridge::convert_to_object(&args);
        assert!(result.is_ok());
        let json_str = result.unwrap();
        assert!(json_str.contains("key1"));
        assert!(json_str.contains("value1"));
    }
}
