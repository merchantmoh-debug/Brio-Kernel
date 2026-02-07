//! Tool system with Type-State pattern and security validation.

use crate::error::{FileSystemError, ToolError};
use crate::types::{ExecutionResult, ToolInvocation, ToolResult};
use regex::{Captures, Regex};
use std::borrow::Cow;
use std::collections::HashMap;
use std::fmt::Write;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

/// Trait for tools that can be executed by agents.
pub trait Tool: Send + Sync {
    /// Returns the unique name of the tool.
    fn name(&self) -> Cow<'static, str>;

    /// Returns the description of the tool in XML format.
    fn description(&self) -> Cow<'static, str>;

    /// Executes the tool with the provided arguments.
    ///
    /// # Errors
    ///
    /// Returns an error if the tool execution fails or the arguments are invalid.
    fn execute(&self, args: &HashMap<String, String>) -> Result<String, ToolError>;
}

/// Type alias for tool argument extractor functions.
pub type ArgExtractor = Box<dyn Fn(&Captures) -> HashMap<String, String> + Send + Sync>;

/// Parser for extracting tool invocations from agent responses.
pub struct ToolParser {
    /// Compiled regex pattern.
    regex: Regex,
    /// Function to extract arguments from captures.
    extractor: ArgExtractor,
}

impl ToolParser {
    /// Creates a new tool parser.
    ///
    /// # Errors
    ///
    /// Returns an error if the regex pattern is invalid.
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

    /// Creates a new tool parser from a known-valid regex pattern.
    ///
    /// # Panics
    ///
    /// Panics if the regex pattern is invalid. This should only be used
    /// with compile-time validated patterns in static initializers.
    #[inline]
    pub fn new_unchecked<E>(pattern: &str, extractor: E) -> Self
    where
        E: Fn(&Captures) -> HashMap<String, String> + Send + Sync + 'static,
    {
        match Self::new(pattern, extractor) {
            Ok(parser) => parser,
            Err(e) => panic!("regex pattern should be valid at compile time: {e}"),
        }
    }

    /// Creates a new tool parser from a pre-compiled regex.
    ///
    /// This is useful when you want to reuse a compiled regex pattern
    /// that was created elsewhere (e.g., in a static OnceLock).
    #[must_use]
    pub fn from_regex<E>(regex: &Regex, extractor: E) -> Self
    where
        E: Fn(&Captures) -> HashMap<String, String> + Send + Sync + 'static,
    {
        Self {
            regex: regex.clone(),
            extractor: Box::new(extractor),
        }
    }

    /// Parses tool invocations from the input text.
    #[must_use]
    pub fn parse(&self, input: &str) -> Vec<ToolInvocation> {
        let mut results = Vec::new();

        for mat in self.regex.find_iter(input) {
            if let Some(caps) = self.regex.captures(mat.as_str()) {
                let args = (self.extractor)(&caps);

                results.push(ToolInvocation {
                    name: Self::extract_tool_name(&caps),
                    args,
                    position: mat.start(),
                });
            }
        }

        // Sort by position to maintain order
        results.sort_by_key(|inv| inv.position);
        results
    }

    fn extract_tool_name(caps: &Captures) -> String {
        // First capture group is typically the tool name
        caps.get(1)
            .map(|m| m.as_str().to_string())
            .unwrap_or_default()
    }
}

/// Registry for managing and executing tools.
pub struct ToolRegistry {
    tools: HashMap<String, Box<dyn Tool>>,
    parsers: HashMap<String, Arc<ToolParser>>,
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
    #[must_use]
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
            parsers: HashMap::new(),
        }
    }

    /// Registers a tool with its parser.
    pub fn register(
        &mut self,
        name: impl Into<String>,
        tool: Box<dyn Tool>,
        parser: impl Into<Arc<ToolParser>>,
    ) {
        let name = name.into();
        self.tools.insert(name.clone(), tool);
        self.parsers.insert(name, parser.into());
    }

    /// Returns a list of available tool names.
    #[must_use]
    pub fn available_tools(&self) -> Vec<&str> {
        self.tools.keys().map(std::string::String::as_str).collect()
    }

    /// Returns help text for all registered tools.
    #[must_use]
    pub fn help_text(&self) -> String {
        self.tools
            .values()
            .map(|t| t.description())
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Executes all tool invocations found in the input.
    ///
    /// # Errors
    ///
    /// Returns an error if a tool is not found or if tool execution fails.
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
                inv.name.clone_from(tool_name);
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
                    let _ = writeln!(
                        collected_output,
                        "✓ {}: {}",
                        invocation.name,
                        result.output.lines().next().unwrap_or(&result.output)
                    );
                    tool_results.push(result);
                }
                Err(e) => {
                    let _ = writeln!(collected_output, "✗ {} failed: {}", invocation.name, e);
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

        match tool.execute(&invocation.args) {
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
///
/// # Errors
///
/// Returns an error if the path contains parent directory references (`..`) or
/// if the resolved path is outside the base directory.
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
            format!("Base directory not found: {e}"),
        ))
    })?;

    // Ensure the resolved path is within the base directory
    if let Ok(canonical_path) = resolved.canonicalize()
        && !canonical_path.starts_with(&canonical_base)
    {
        return Err(FileSystemError::PathTraversal {
            path: path.to_path_buf(),
        });
    }

    Ok(resolved)
}

/// Checks if a file size is within limits.
///
/// # Errors
///
/// Returns an error if the file metadata cannot be read or if the file size
/// exceeds the specified maximum.
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
///
/// # Errors
///
/// Returns an error if the command is not in the allowlist or if it contains
/// potentially dangerous characters (e.g., `;`, `&`, `|`, `>`, `<`, `` ` ``, `$`, `(`).
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
            reason: format!("Command '{first_word}' is not in the allowed list"),
        });
    }

    // Additional security: check for dangerous characters
    let dangerous_chars = [b';', b'&', b'|', b'>', b'<', b'`', b'$', b'('];
    if command.bytes().any(|c| dangerous_chars.contains(&c)) {
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
pub struct Validated;

impl SecureFilePath<Unvalidated> {
    /// Creates a new unvalidated file path.
    #[must_use]
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            _state: std::marker::PhantomData,
        }
    }

    /// Validates the path against a base directory.
    ///
    /// # Errors
    ///
    /// Returns an error if the path contains parent directory references or
    /// if the resolved path is outside the base directory.
    pub fn validate(self, base_dir: &Path) -> Result<SecureFilePath<Validated>, FileSystemError> {
        let path_str = self.path.to_str().ok_or_else(|| {
            FileSystemError::InvalidPath(format!(
                "Path contains invalid UTF-8: {}",
                self.path.display()
            ))
        })?;
        let validated_path = validate_path(path_str, base_dir)?;

        Ok(SecureFilePath {
            path: validated_path,
            _state: std::marker::PhantomData,
        })
    }
}

impl SecureFilePath<Validated> {
    /// Returns the validated path.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Reads the file content with size validation.
    ///
    /// # Errors
    ///
    /// Returns an error if the file size exceeds the limit or if the file
    /// cannot be read.
    pub fn read_with_limit(&self, max_size: u64) -> Result<String, FileSystemError> {
        validate_file_size(&self.path, max_size)?;
        std::fs::read_to_string(&self.path).map_err(FileSystemError::Io)
    }

    /// Writes content to the file.
    ///
    /// # Errors
    ///
    /// Returns an error if parent directories cannot be created or if the file
    /// cannot be written.
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
        struct TestTool;
        impl Tool for TestTool {
            fn name(&self) -> Cow<'static, str> {
                Cow::Borrowed("test")
            }
            fn description(&self) -> Cow<'static, str> {
                Cow::Borrowed("<test />")
            }
            fn execute(&self, _args: &HashMap<String, String>) -> Result<String, ToolError> {
                Ok("test result".to_string())
            }
        }

        let mut registry = ToolRegistry::new();

        let parser = Arc::new(
            ToolParser::new(r"\u003ctest\s*/?\u003e", |_caps: &Captures| {
                let mut args = HashMap::new();
                args.insert("arg".to_string(), "value".to_string());
                args
            })
            .unwrap(),
        );

        registry.register("test", Box::new(TestTool), parser);
        assert_eq!(registry.available_tools(), vec!["test"]);
    }
}
