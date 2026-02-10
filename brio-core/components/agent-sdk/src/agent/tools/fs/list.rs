//! Directory list tool for agent operations.

use crate::agent::tools::fs::get_base_dir;
use crate::error::ToolError;
use crate::tools::constants::fs;
use crate::tools::{Tool, validate_path};
use std::borrow::Cow;
use std::collections::HashMap;

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
        Cow::Borrowed(fs::LS)
    }

    fn description(&self) -> Cow<'static, str> {
        Cow::Borrowed(r#"<ls path="path/to/directory" /> - List directory contents"#)
    }

    fn execute(&self, args: &HashMap<String, String>) -> Result<String, ToolError> {
        let path_str = args
            .get("path")
            .ok_or_else(|| ToolError::InvalidArguments {
                tool: fs::LS.to_string(),
                reason: "Missing 'path' argument".to_string(),
            })?;

        let base_dir = get_base_dir(fs::LS)?;

        // Validate path to prevent traversal attacks
        let path = validate_path(path_str, &base_dir).map_err(|e| ToolError::ExecutionFailed {
            tool: fs::LS.to_string(),
            source: Box::new(e),
        })?;

        // Read directory entries
        let entries: Vec<String> = std::fs::read_dir(&path)
            .map_err(|e| ToolError::ExecutionFailed {
                tool: fs::LS.to_string(),
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
    use std::collections::HashMap;

    #[test]
    fn test_list_directory_tool_name() {
        let tool = ListDirectoryTool::new(10);
        assert_eq!(tool.name(), fs::LS);
    }

    #[test]
    fn test_list_directory_missing_path() {
        let tool = ListDirectoryTool::new(10);
        let args = HashMap::new();
        let result = tool.execute(&args);

        assert!(matches!(
            result,
            Err(ToolError::InvalidArguments { tool, reason })
            if tool == fs::LS && reason.contains("path")
        ));
    }
}
