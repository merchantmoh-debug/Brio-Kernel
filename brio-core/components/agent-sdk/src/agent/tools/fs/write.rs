//! File write tool for agent operations.

use crate::agent::tools::fs::get_base_dir;
use crate::error::ToolError;
use crate::tools::constants::fs;
use crate::tools::{Tool, validate_path};
use std::borrow::Cow;
use std::collections::HashMap;

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
        Cow::Borrowed(fs::WRITE_FILE)
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
                tool: fs::WRITE_FILE.to_string(),
                reason: "Missing 'path' argument".to_string(),
            })?;

        let content = args
            .get("content")
            .ok_or_else(|| ToolError::InvalidArguments {
                tool: fs::WRITE_FILE.to_string(),
                reason: "Missing 'content' argument".to_string(),
            })?;

        let base_dir = get_base_dir(fs::WRITE_FILE)?;

        // Validate path to prevent traversal attacks
        let path = validate_path(path_str, &base_dir).map_err(|e| ToolError::ExecutionFailed {
            tool: fs::WRITE_FILE.to_string(),
            source: Box::new(e),
        })?;

        // Create parent directories if needed
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| ToolError::ExecutionFailed {
                tool: fs::WRITE_FILE.to_string(),
                source: Box::new(e),
            })?;
        }

        // Write file content
        std::fs::write(&path, content).map_err(|e| ToolError::ExecutionFailed {
            tool: fs::WRITE_FILE.to_string(),
            source: Box::new(e),
        })?;

        Ok(format!(
            "Wrote {} bytes to {}",
            content.len(),
            path.display()
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_write_file_tool_name() {
        let tool = WriteFileTool;
        assert_eq!(tool.name(), fs::WRITE_FILE);
    }

    #[test]
    fn test_write_file_missing_path() {
        let tool = WriteFileTool;
        let args = HashMap::new();
        let result = tool.execute(&args);

        assert!(matches!(
            result,
            Err(ToolError::InvalidArguments { tool, reason })
            if tool == fs::WRITE_FILE && reason.contains("path")
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
            if tool == fs::WRITE_FILE && reason.contains("content")
        ));
    }
}
