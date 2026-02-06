//! Tool system with Type-State pattern and security validation.

use crate::error::{FileSystemError, ToolError};
use crate::types::{ExecutionResult, ToolInvocation, ToolResult};
use regex::{Captures, Regex};
use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};
use std::time::Instant;

/// Trait for tools that can be executed by agents.
pub trait Tool: Send + Sync {
    /// Returns the unique name of the tool.
    fn name(&self) -> &str;

    /// Returns the description of the tool in XML format.
    fn description(&self) -> &str;

    /// Executes the tool with the provided arguments.
    fn execute(&self, args: HashMap<String, String>) -> Result<String, ToolError>;
}

/// Parser for extracting tool invocations from agent responses.
pub struct ToolParser {
    /// Compiled regex pattern.
    regex: Regex,
    /// Function to extract arguments from captures.
    extractor: Box<dyn Fn(&Captures) -> HashMap<String, String> + Send + Sync>,
}

impl ToolParser {
    /// Creates a new tool parser.
    pub fn new<E>(pattern: &str, extractor: E) -> Result<Self, regex::Error>
    where
        E: Fn(&Captures) -> HashMap<String, String> + Send + Sync + 'static,
    {
        let regex = Regex::new(pattern)?;
        Ok(Self {
            regex,
            extractor: Box::new(extractor),
        })
    }

    /// Parses tool invocations from the input text.
    pub fn parse(&self, input: &str) -> Vec<ToolInvocation> {
        let mut results = Vec::new();

        for mat in self.regex.find_iter(input) {
            if let Some(caps) = self.regex.captures(mat.as_str()) {
                let args = (self.extractor)(&caps);

                results.push(ToolInvocation {
                    name: self.extract_tool_name(&caps),
                    args,
                    position: mat.start(),
                });
            }
        }

        // Sort by position to maintain order
        results.sort_by_key(|inv| inv.position);
        results
    }

    fn extract_tool_name(&self, caps: &Captures) -> String {
        // First capture group is typically the tool name
        caps.get(1)
            .map(|m| m.as_str().to_string())
            .unwrap_or_default()
    }
}

/// Registry for managing and executing tools.
pub struct ToolRegistry {
    tools: HashMap<String, Box<dyn Tool>>,
    parsers: HashMap<String, ToolParser>,
}

impl std::fmt::Debug for ToolRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ToolRegistry")
            .field("tools", &self.tools.keys().collect::<Vec<_>>())
            .field("parsers", &self.parsers.keys().collect::<Vec<_>>())
            .finish()
    }
}

impl ToolRegistry {
    /// Creates a new empty tool registry.
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
            parsers: HashMap::new(),
        }
    }

    /// Registers a tool with its parser.
    pub fn register(&mut self, name: impl Into<String>, tool: Box<dyn Tool>, parser: ToolParser) {
        let name = name.into();
        self.tools.insert(name.clone(), tool);
        self.parsers.insert(name, parser);
    }

    /// Returns a list of available tool names.
    pub fn available_tools(&self) -> Vec<&str> {
        self.tools.keys().map(|s| s.as_str()).collect()
    }

    /// Returns help text for all registered tools.
    pub fn help_text(&self) -> String {
        self.tools
            .values()
            .map(|t| t.description())
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Executes all tool invocations found in the input.
    pub fn execute_all(&self, input: &str) -> Result<ExecutionResult, ToolError> {
        let mut collected_output = String::new();
        let mut is_done = false;
        let mut final_summary = None;
        let mut tool_results = Vec::new();

        // Collect all invocations
        let mut invocations: Vec<ToolInvocation> = Vec::new();
        for (tool_name, parser) in &self.parsers {
            let mut parsed = parser.parse(input);
            for inv in &mut parsed {
                inv.name = tool_name.clone();
            }
            invocations.extend(parsed);
        }

        // Sort by position
        invocations.sort_by_key(|inv| inv.position);

        // Execute each invocation
        for invocation in invocations {
            if invocation.name == "done" {
                is_done = true;
                if let Some(summary) = invocation.args.get("summary") {
                    final_summary = Some(summary.clone());
                }
                break;
            }

            match self.execute_single(&invocation) {
                Ok(result) => {
                    collected_output.push_str(&format!(
                        "✓ {}: {}\n",
                        invocation.name,
                        result.output.lines().next().unwrap_or(&result.output)
                    ));
                    tool_results.push(result);
                }
                Err(e) => {
                    collected_output.push_str(&format!("✗ {} failed: {}\n", invocation.name, e));
                    return Err(e);
                }
            }
        }

        Ok(ExecutionResult {
            output: collected_output,
            is_complete: is_done,
            summary: final_summary,
            tool_results,
        })
    }

    /// Executes a single tool invocation.
    fn execute_single(&self, invocation: &ToolInvocation) -> Result<ToolResult, ToolError> {
        let tool = self
            .tools
            .get(&invocation.name)
            .ok_or_else(|| ToolError::NotFound {
                name: invocation.name.clone(),
            })?;

        let start = Instant::now();

        match tool.execute(invocation.args.clone()) {
            Ok(output) => Ok(ToolResult {
                success: true,
                output,
                duration: start.elapsed(),
            }),
            Err(e) => Err(ToolError::ExecutionFailed {
                tool: invocation.name.clone(),
                source: Box::new(e),
            }),
        }
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Validates a file path to prevent path traversal attacks.
pub fn validate_path(path: &str, base_dir: &Path) -> Result<PathBuf, FileSystemError> {
    let path = Path::new(path);

    // Check for path traversal components
    for component in path.components() {
        if let Component::ParentDir = component {
            return Err(FileSystemError::PathTraversal {
                path: path.to_path_buf(),
            });
        }
    }

    // Resolve the path relative to base directory
    let resolved = base_dir.join(path);
    let canonical_base = base_dir.canonicalize().map_err(|e| {
        FileSystemError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("Base directory not found: {}", e),
        ))
    })?;

    // Ensure the resolved path is within the base directory
    if let Ok(canonical_path) = resolved.canonicalize() {
        if !canonical_path.starts_with(&canonical_base) {
            return Err(FileSystemError::PathTraversal {
                path: path.to_path_buf(),
            });
        }
    }

    Ok(resolved)
}

/// Checks if a file size is within limits.
pub fn validate_file_size(path: &Path, max_size: u64) -> Result<(), FileSystemError> {
    let metadata = std::fs::metadata(path).map_err(FileSystemError::Io)?;
    let size = metadata.len();

    if size > max_size {
        return Err(FileSystemError::FileTooLarge {
            path: path.to_path_buf(),
            size,
            max_size,
        });
    }

    Ok(())
}

/// Validates a shell command against an allowlist.
pub fn validate_shell_command(
    command: &str,
    allowlist: &[impl AsRef<str>],
) -> Result<(), ToolError> {
    let cmd_trimmed = command.trim();
    let first_word = cmd_trimmed.split_whitespace().next().unwrap_or(cmd_trimmed);

    let is_allowed = allowlist.iter().any(|prefix| first_word == prefix.as_ref());

    if !is_allowed {
        return Err(ToolError::Blocked {
            tool: "shell".to_string(),
            reason: format!("Command '{}' is not in the allowed list", first_word),
        });
    }

    // Additional security: check for dangerous characters
    let dangerous_chars = [';', '&', '|', '>', '<', '`', '$', '('];
    if command.chars().any(|c| dangerous_chars.contains(&c)) {
        return Err(ToolError::Blocked {
            tool: "shell".to_string(),
            reason: "Command contains potentially dangerous characters".to_string(),
        });
    }

    Ok(())
}

/// Type-State pattern for secure file operations.
///
/// This ensures that file paths are validated before any operations are performed.
pub struct SecureFilePath<State> {
    path: PathBuf,
    _state: std::marker::PhantomData<State>,
}

/// Unvalidated state.
pub struct Unvalidated;

/// Validated state.
pub struct Validated {
    #[allow(dead_code)]
    base_dir: PathBuf,
}

impl SecureFilePath<Unvalidated> {
    /// Creates a new unvalidated file path.
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            _state: std::marker::PhantomData,
        }
    }

    /// Validates the path against a base directory.
    pub fn validate(self, base_dir: &Path) -> Result<SecureFilePath<Validated>, FileSystemError> {
        let validated_path = validate_path(self.path.to_str().unwrap_or(""), base_dir)?;

        Ok(SecureFilePath {
            path: validated_path,
            _state: std::marker::PhantomData,
        })
    }
}

impl SecureFilePath<Validated> {
    /// Returns the validated path.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Reads the file content with size validation.
    pub fn read_with_limit(&self, max_size: u64) -> Result<String, FileSystemError> {
        validate_file_size(&self.path, max_size)?;
        std::fs::read_to_string(&self.path).map_err(FileSystemError::Io)
    }

    /// Writes content to the file.
    pub fn write(&self, content: &str) -> Result<(), FileSystemError> {
        // Create parent directories if needed
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent).map_err(FileSystemError::Io)?;
        }
        std::fs::write(&self.path, content).map_err(FileSystemError::Io)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_validate_path_traversal() {
        let base = Path::new("/tmp/test");
        let result = validate_path("../../../etc/passwd", base);
        assert!(matches!(result, Err(FileSystemError::PathTraversal { .. })));
    }

    #[test]
    fn test_validate_shell_command() {
        let allowlist = vec!["ls", "cat", "echo"];

        assert!(validate_shell_command("ls -la", &allowlist).is_ok());
        assert!(validate_shell_command("rm -rf /", &allowlist).is_err());
        assert!(validate_shell_command("ls; rm -rf /", &allowlist).is_err());
    }

    #[test]
    fn test_secure_file_path() {
        let temp_dir = tempfile::tempdir().unwrap();
        let base = temp_dir.path();

        // Create a test file
        let test_file = base.join("test.txt");
        std::fs::write(&test_file, "Hello").unwrap();

        // Test validation
        let secure = SecureFilePath::new("test.txt").validate(base).unwrap();
        let content = secure.read_with_limit(1024).unwrap();
        assert_eq!(content, "Hello");
    }

    #[test]
    fn test_tool_registry() {
        let mut registry = ToolRegistry::new();

        struct TestTool;
        impl Tool for TestTool {
            fn name(&self) -> &str {
                "test"
            }
            fn description(&self) -> &str {
                "<test />"
            }
            fn execute(&self, _args: HashMap<String, String>) -> Result<String, ToolError> {
                Ok("test result".to_string())
            }
        }

        let parser = ToolParser::new(r"\u003ctest\s*/?\u003e", |_caps: &Captures| {
            let mut args = HashMap::new();
            args.insert("arg".to_string(), "value".to_string());
            args
        })
        .unwrap();

        registry.register("test", Box::new(TestTool), parser);
        assert_eq!(registry.available_tools(), vec!["test"]);
    }
}
