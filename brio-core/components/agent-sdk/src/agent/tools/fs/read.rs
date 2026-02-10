//! File read tool for agent operations.

use crate::agent::tools::fs::get_base_dir;
use crate::error::{FileSystemError, ToolError};
use crate::tools::constants::fs;
use crate::tools::{Tool, validate_file_size, validate_path};
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
        Cow::Borrowed(fs::READ_FILE)
    }

    fn description(&self) -> Cow<'static, str> {
        Cow::Borrowed(r#"<read_file path="path/to/file" /> - Read content from a file"#)
    }

    fn execute(&self, args: &HashMap<String, String>) -> Result<String, ToolError> {
        let path_str = args
            .get("path")
            .ok_or_else(|| ToolError::InvalidArguments {
                tool: fs::READ_FILE.to_string(),
                reason: "Missing 'path' argument".to_string(),
            })?;

        let base_dir = get_base_dir(fs::READ_FILE)?;

        // Validate path to prevent traversal attacks
        let path = validate_path(path_str, &base_dir).map_err(|e| ToolError::ExecutionFailed {
            tool: fs::READ_FILE.to_string(),
            source: Box::new(e),
        })?;

        // Check file size before reading
        validate_file_size(&path, self.max_size).map_err(|e| match e {
            FileSystemError::FileTooLarge { size, max_size, .. } => {
                ToolError::ResourceLimitExceeded {
                    tool: fs::READ_FILE.to_string(),
                    resource: format!("file size ({size} bytes, max: {max_size})"),
                }
            }
            _ => ToolError::ExecutionFailed {
                tool: fs::READ_FILE.to_string(),
                source: Box::new(e),
            },
        })?;

        // Read file content
        std::fs::read_to_string(&path).map_err(|e| ToolError::ExecutionFailed {
            tool: fs::READ_FILE.to_string(),
            source: Box::new(e),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_read_file_tool_name() {
        let tool = ReadFileTool::new(1024);
        assert_eq!(tool.name(), fs::READ_FILE);
    }

    #[test]
    fn test_read_file_missing_path() {
        let tool = ReadFileTool::new(1024);
        let args = HashMap::new();
        let result = tool.execute(&args);

        assert!(matches!(
            result,
            Err(ToolError::InvalidArguments { tool, reason })
            if tool == fs::READ_FILE && reason.contains("path")
        ));
    }
}
