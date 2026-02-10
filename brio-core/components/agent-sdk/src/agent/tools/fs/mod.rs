//! File system tools for agent operations.
//!
//! This module provides secure file system operations with path validation
//! to prevent directory traversal attacks. All tools implement proper
//! error handling and resource limits.

pub mod list;
pub mod read;
pub mod write;

use crate::error::ToolError;
use std::path::PathBuf;

/// Helper function to get the current working directory.
pub(crate) fn get_base_dir(tool_name: &str) -> Result<PathBuf, ToolError> {
    std::env::current_dir().map_err(|e| ToolError::ExecutionFailed {
        tool: tool_name.to_string(),
        source: Box::new(e),
    })
}

pub use list::ListDirectoryTool;
pub use read::ReadFileTool;
pub use write::WriteFileTool;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::Tool;
    use std::collections::HashMap;

    // Use the shared mutex from lib.rs to serialize directory-changing tests

    fn setup_test_dir() -> tempfile::TempDir {
        tempfile::tempdir().expect("Failed to create temp directory")
    }

    #[test]
    fn test_write_and_read_file() {
        // Serialize directory-changing tests to avoid race conditions
        let _guard = crate::DIR_MUTEX.lock().unwrap();

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
        // Serialize directory-changing tests to avoid race conditions
        let _guard = crate::DIR_MUTEX.lock().unwrap();

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
        // Serialize directory-changing tests to avoid race conditions
        let _guard = crate::DIR_MUTEX.lock().unwrap();

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
