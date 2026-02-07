//! File system tools for agent operations.
//!
//! This module provides secure file system operations with path validation
//! to prevent directory traversal attacks. All tools implement proper
//! error handling and resource limits.

use crate::error::{FileSystemError, ToolError};
use crate::tools::{validate_file_size, validate_path, Tool};
use std::borrow::Cow;
use std::collections::HashMap;

/// Tool for reading file contents.
///
/// Reads the content of a file after validating the path and checking
/// that the file size does not exceed the configured maximum.
///
/// # Security
///
/// - Validates path to prevent directory traversal attacks
/// - Enforces maximum file size limits
/// - Returns appropriate errors for missing or inaccessible files
///
/// # Example
///
/// ```
/// use agent_sdk::agent::tools::ReadFileTool;
/// use agent_sdk::Tool;
/// use std::collections::HashMap;
///
/// # fn example() {
/// let tool = ReadFileTool::new(1024 * 1024); // 1MB max
/// # }
/// ```
pub struct ReadFileTool {
    max_size: u64,
}

impl ReadFileTool {
    /// Creates a new `ReadFileTool` with the specified maximum file size.
    ///
    /// # Arguments
    ///
    /// * `max_size` - Maximum file size in bytes that can be read
    #[must_use]
    pub fn new(max_size: u64) -> Self {
        Self { max_size }
    }
}

impl Tool for ReadFileTool {
    fn name(&self) -> Cow<'static, str> {
        Cow::Borrowed("read_file")
    }

    fn description(&self) -> Cow<'static, str> {
        Cow::Borrowed(r#"<read_file path="path/to/file" /> - Read content from a file"#)
    }

    fn execute(&self, args: &HashMap<String, String>) -> Result<String, ToolError> {
        let path_str = args
            .get("path")
            .ok_or_else(|| ToolError::InvalidArguments {
                tool: "read_file".to_string(),
                reason: "Missing 'path' argument".to_string(),
            })?;

        // Get current working directory as base
        let base_dir = std::env::current_dir().map_err(|e| ToolError::ExecutionFailed {
            tool: "read_file".to_string(),
            source: Box::new(e),
        })?;

        // Validate path to prevent traversal attacks
        let path = validate_path(path_str, &base_dir).map_err(|e| ToolError::ExecutionFailed {
            tool: "read_file".to_string(),
            source: Box::new(e),
        })?;

        // Check file size before reading
        validate_file_size(&path, self.max_size).map_err(|e| match e {
            FileSystemError::FileTooLarge { size, max_size, .. } => {
                ToolError::ResourceLimitExceeded {
                    tool: "read_file".to_string(),
                    resource: format!("file size ({size} bytes, max: {max_size})"),
                }
            }
            _ => ToolError::ExecutionFailed {
                tool: "read_file".to_string(),
                source: Box::new(e),
            },
        })?;

        // Read file content
        std::fs::read_to_string(&path).map_err(|e| ToolError::ExecutionFailed {
            tool: "read_file".to_string(),
            source: Box::new(e),
        })
    }
}

/// Tool for writing file contents.
///
/// Writes content to a file after validating the path. Parent directories
/// are created automatically if they don't exist.
///
/// # Security
///
/// - Validates path to prevent directory traversal attacks
/// - Creates parent directories with safe permissions
/// - Returns appropriate errors for permission issues
///
/// # Example
///
/// ```
/// use agent_sdk::agent::tools::WriteFileTool;
/// use agent_sdk::Tool;
/// use std::collections::HashMap;
///
/// # fn example() {
/// let tool = WriteFileTool;
/// # }
/// ```
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
        let path_str = args
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

        // Get current working directory as base
        let base_dir = std::env::current_dir().map_err(|e| ToolError::ExecutionFailed {
            tool: "write_file".to_string(),
            source: Box::new(e),
        })?;

        // Validate path to prevent traversal attacks
        let path = validate_path(path_str, &base_dir).map_err(|e| ToolError::ExecutionFailed {
            tool: "write_file".to_string(),
            source: Box::new(e),
        })?;

        // Create parent directories if needed
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| ToolError::ExecutionFailed {
                tool: "write_file".to_string(),
                source: Box::new(e),
            })?;
        }

        // Write file content
        std::fs::write(&path, content).map_err(|e| ToolError::ExecutionFailed {
            tool: "write_file".to_string(),
            source: Box::new(e),
        })?;

        Ok(format!(
            "Wrote {} bytes to {}",
            content.len(),
            path.display()
        ))
    }
}

/// Tool for listing directory contents.
///
/// Lists the contents of a directory after validating the path.
/// Returns a formatted list showing file types and names.
///
/// # Security
///
/// - Validates path to prevent directory traversal attacks
/// - Returns appropriate errors for missing or inaccessible directories
///
/// # Example
///
/// ```
/// use agent_sdk::agent::tools::ListDirectoryTool;
/// use agent_sdk::Tool;
/// use std::collections::HashMap;
///
/// # fn example() {
/// let tool = ListDirectoryTool::new(10); // max_depth for future use
/// # }
/// ```
pub struct ListDirectoryTool {
    /// Maximum depth for directory traversal (reserved for future use).
    #[expect(dead_code)]
    max_depth: usize,
}

impl ListDirectoryTool {
    /// Creates a new `ListDirectoryTool` with the specified maximum depth.
    ///
    /// # Arguments
    ///
    /// * `max_depth` - Maximum depth for directory traversal (reserved for future use)
    #[must_use]
    pub fn new(max_depth: usize) -> Self {
        Self { max_depth }
    }
}

impl Tool for ListDirectoryTool {
    fn name(&self) -> Cow<'static, str> {
        Cow::Borrowed("ls")
    }

    fn description(&self) -> Cow<'static, str> {
        Cow::Borrowed(r#"<ls path="path/to/directory" /> - List directory contents"#)
    }

    fn execute(&self, args: &HashMap<String, String>) -> Result<String, ToolError> {
        let path_str = args
            .get("path")
            .ok_or_else(|| ToolError::InvalidArguments {
                tool: "ls".to_string(),
                reason: "Missing 'path' argument".to_string(),
            })?;

        // Get current working directory as base
        let base_dir = std::env::current_dir().map_err(|e| ToolError::ExecutionFailed {
            tool: "ls".to_string(),
            source: Box::new(e),
        })?;

        // Validate path to prevent traversal attacks
        let path = validate_path(path_str, &base_dir).map_err(|e| ToolError::ExecutionFailed {
            tool: "ls".to_string(),
            source: Box::new(e),
        })?;

        // Read directory entries
        let entries: Vec<String> = std::fs::read_dir(&path)
            .map_err(|e| ToolError::ExecutionFailed {
                tool: "ls".to_string(),
                source: Box::new(e),
            })?
            .filter_map(std::result::Result::ok)
            .map(|entry| {
                let name = entry.file_name().to_string_lossy().to_string();
                let ty = if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                    "[DIR]"
                } else {
                    "[FILE]"
                };
                format!("{ty} {name}")
            })
            .collect();

        if entries.is_empty() {
            Ok("Empty directory".to_string())
        } else {
            Ok(entries.join("\n"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_test_dir() -> tempfile::TempDir {
        tempfile::tempdir().expect("Failed to create temp directory")
    }

    #[test]
    fn test_read_file_tool_name() {
        let tool = ReadFileTool::new(1024);
        assert_eq!(tool.name(), "read_file");
    }

    #[test]
    fn test_write_file_tool_name() {
        let tool = WriteFileTool;
        assert_eq!(tool.name(), "write_file");
    }

    #[test]
    fn test_list_directory_tool_name() {
        let tool = ListDirectoryTool::new(10);
        assert_eq!(tool.name(), "ls");
    }

    #[test]
    fn test_read_file_missing_path() {
        let tool = ReadFileTool::new(1024);
        let args = HashMap::new();
        let result = tool.execute(&args);

        assert!(matches!(
            result,
            Err(ToolError::InvalidArguments { tool, reason })
            if tool == "read_file" && reason.contains("path")
        ));
    }

    #[test]
    fn test_write_file_missing_path() {
        let tool = WriteFileTool;
        let args = HashMap::new();
        let result = tool.execute(&args);

        assert!(matches!(
            result,
            Err(ToolError::InvalidArguments { tool, reason })
            if tool == "write_file" && reason.contains("path")
        ));
    }

    #[test]
    fn test_write_file_missing_content() {
        let tool = WriteFileTool;
        let mut args = HashMap::new();
        args.insert("path".to_string(), "test.txt".to_string());
        let result = tool.execute(&args);

        assert!(matches!(
            result,
            Err(ToolError::InvalidArguments { tool, reason })
            if tool == "write_file" && reason.contains("content")
        ));
    }

    #[test]
    fn test_list_directory_missing_path() {
        let tool = ListDirectoryTool::new(10);
        let args = HashMap::new();
        let result = tool.execute(&args);

        assert!(matches!(
            result,
            Err(ToolError::InvalidArguments { tool, reason })
            if tool == "ls" && reason.contains("path")
        ));
    }

    #[test]
    fn test_write_and_read_file() {
        let temp_dir = setup_test_dir();
        let base_path = temp_dir.path();

        // Change to temp directory for the test
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(base_path).unwrap();

        let write_tool = WriteFileTool;
        let mut args = HashMap::new();
        args.insert("path".to_string(), "test.txt".to_string());
        args.insert("content".to_string(), "Hello, World!".to_string());

        let write_result = write_tool.execute(&args).unwrap();
        assert!(write_result.contains("13 bytes"));

        let read_tool = ReadFileTool::new(1024);
        let mut args = HashMap::new();
        args.insert("path".to_string(), "test.txt".to_string());

        let read_result = read_tool.execute(&args).unwrap();
        assert_eq!(read_result, "Hello, World!");

        // Restore original directory
        std::env::set_current_dir(original_dir).unwrap();
    }

    #[test]
    fn test_list_directory() {
        let temp_dir = setup_test_dir();
        let base_path = temp_dir.path();

        // Change to temp directory for the test
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(base_path).unwrap();

        // Create test files
        std::fs::write(base_path.join("file1.txt"), "content1").unwrap();
        std::fs::write(base_path.join("file2.txt"), "content2").unwrap();
        std::fs::create_dir(base_path.join("subdir")).unwrap();

        let list_tool = ListDirectoryTool::new(10);
        let mut args = HashMap::new();
        args.insert("path".to_string(), ".".to_string());

        let result = list_tool.execute(&args).unwrap();
        assert!(result.contains("[FILE] file1.txt"));
        assert!(result.contains("[FILE] file2.txt"));
        assert!(result.contains("[DIR] subdir"));

        // Restore original directory
        std::env::set_current_dir(original_dir).unwrap();
    }

    #[test]
    fn test_path_traversal_blocked() {
        let temp_dir = setup_test_dir();
        let base_path = temp_dir.path();

        // Change to temp directory for the test
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(base_path).unwrap();

        let read_tool = ReadFileTool::new(1024);
        let mut args = HashMap::new();
        args.insert("path".to_string(), "../../../etc/passwd".to_string());

        let result = read_tool.execute(&args);
        assert!(result.is_err());

        // Restore original directory
        std::env::set_current_dir(original_dir).unwrap();
    }
}
