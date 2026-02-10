//! Grep tool for searching file contents with regex support.
//!
//! This module provides secure file searching with regex pattern matching.
//! It supports both single file and directory searching with configurable
//! options for case sensitivity and result limits.

use crate::agent::tools::fs::get_base_dir;
use crate::error::{FileSystemError, ToolError};
use crate::tools::constants::grep;
use crate::tools::{Tool, validate_file_size, validate_path};
use regex::RegexBuilder;
use std::borrow::Cow;
use std::collections::HashMap;
use std::path::Path;

/// Maximum file size to search (10MB).
const MAX_FILE_SIZE: u64 = 10 * 1024 * 1024;

/// Default maximum number of results to return.
const DEFAULT_MAX_RESULTS: usize = 1000;

/// Maximum pattern size to prevent memory exhaustion (100KB).
const MAX_PATTERN_SIZE: usize = 100 * 1024;

/// A single grep match result.
#[derive(Debug, Clone)]
struct GrepMatch {
    file_path: String,
    line_number: usize,
    content: String,
}

/// Tool for searching file contents using regex patterns.
///
/// Searches files for matches to a regex pattern, supporting both
/// single file and recursive directory searches. Results are returned
/// in a standardized format with file path, line number, and matched content.
///
/// # Security
///
/// - Validates all paths to prevent directory traversal attacks
/// - Enforces maximum file size limits (10MB per file)
/// - Limits total number of results (default 1000)
/// - Regex compilation errors are handled gracefully
///
/// # Example
///
/// ```
/// use agent_sdk::agent::tools::GrepTool;
/// use agent_sdk::Tool;
/// use std::collections::HashMap;
///
/// # fn example() {
/// let tool = GrepTool::new();
/// # }
/// ```
pub struct GrepTool;

impl GrepTool {
    /// Creates a new `GrepTool` with default settings.
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// Parses optional boolean argument from string.
    fn parse_bool_arg(value: &str, default: bool) -> bool {
        value.parse::<bool>().unwrap_or(default)
    }

    /// Parses optional usize argument from string.
    fn parse_usize_arg(value: &str, default: usize) -> usize {
        value.parse::<usize>().unwrap_or(default)
    }

    /// Compiles regex pattern with given options.
    fn compile_pattern(pattern: &str, case_insensitive: bool) -> Result<regex::Regex, ToolError> {
        // Check pattern size to prevent memory exhaustion
        if pattern.len() > MAX_PATTERN_SIZE {
            return Err(ToolError::InvalidArguments {
                tool: grep::GREP.to_string(),
                reason: format!(
                    "Pattern too large: {} bytes (max: {})",
                    pattern.len(),
                    MAX_PATTERN_SIZE
                ),
            });
        }

        RegexBuilder::new(pattern)
            .case_insensitive(case_insensitive)
            .dfa_size_limit(10 * 1024 * 1024) // 10MB limit for DFA to prevent ReDoS
            .build()
            .map_err(|e| ToolError::InvalidArguments {
                tool: grep::GREP.to_string(),
                reason: format!("Invalid regex pattern: {e}"),
            })
    }

    /// Searches a single file for pattern matches.
    fn search_file(
        &self,
        file_path: &Path,
        pattern: &regex::Regex,
        max_results: usize,
        current_count: &mut usize,
        matches: &mut Vec<GrepMatch>,
    ) -> Result<(), FileSystemError> {
        // Validate file size before reading
        validate_file_size(file_path, MAX_FILE_SIZE)?;

        let content = std::fs::read_to_string(file_path).map_err(FileSystemError::Io)?;

        for (line_num, line) in content.lines().enumerate() {
            if *current_count >= max_results {
                break;
            }

            if pattern.is_match(line) {
                matches.push(GrepMatch {
                    file_path: file_path.to_string_lossy().to_string(),
                    line_number: line_num + 1, // 1-based line numbers
                    content: line.to_string(),
                });
                *current_count += 1;
            }
        }

        Ok(())
    }

    /// Recursively searches directory for pattern matches.
    fn search_directory(
        &self,
        dir_path: &Path,
        pattern: &regex::Regex,
        max_results: usize,
        current_count: &mut usize,
        matches: &mut Vec<GrepMatch>,
    ) -> Result<(), FileSystemError> {
        for entry in walkdir::WalkDir::new(dir_path)
            .follow_links(false)
            .into_iter()
            .filter_map(std::result::Result::ok)
        {
            if *current_count >= max_results {
                break;
            }

            let path = entry.path();
            if path.is_file() {
                // Skip files that are too large or can't be read as text
                if let Err(e) = validate_file_size(path, MAX_FILE_SIZE) {
                    tracing::debug!("Skipping file {}: {}", path.display(), e);
                    continue;
                }

                // Attempt to search the file, skip binary files silently
                if let Err(e) = self.search_file(path, pattern, max_results, current_count, matches)
                {
                    tracing::debug!("Skipping file {}: {}", path.display(), e);
                }
            }
        }

        Ok(())
    }

    /// Formats matches into the standard output format.
    fn format_results(matches: &[GrepMatch]) -> String {
        if matches.is_empty() {
            return "No matches found".to_string();
        }

        matches
            .iter()
            .map(|m| format!("{}:{}:{}", m.file_path, m.line_number, m.content))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

impl Default for GrepTool {
    fn default() -> Self {
        Self::new()
    }
}

impl Tool for GrepTool {
    fn name(&self) -> Cow<'static, str> {
        Cow::Borrowed(grep::GREP)
    }

    fn description(&self) -> Cow<'static, str> {
        Cow::Borrowed(
            r#"<grep pattern="regex" path="file_or_dir" [case_insensitive="true|false"] [max_results="1000"] /> - Search files with regex"#,
        )
    }

    fn execute(&self, args: &HashMap<String, String>) -> Result<String, ToolError> {
        // Extract required arguments
        let pattern_str = args
            .get("pattern")
            .ok_or_else(|| ToolError::InvalidArguments {
                tool: grep::GREP.to_string(),
                reason: "Missing 'pattern' argument".to_string(),
            })?;

        let path_str = args
            .get("path")
            .ok_or_else(|| ToolError::InvalidArguments {
                tool: grep::GREP.to_string(),
                reason: "Missing 'path' argument".to_string(),
            })?;

        // Extract optional arguments with defaults
        let case_insensitive = args
            .get("case_insensitive")
            .is_some_and(|v| Self::parse_bool_arg(v, false));

        let max_results = args.get("max_results").map_or(DEFAULT_MAX_RESULTS, |v| {
            Self::parse_usize_arg(v, DEFAULT_MAX_RESULTS)
        });

        // Compile regex pattern
        let pattern = Self::compile_pattern(pattern_str, case_insensitive)?;

        // Get base directory for path validation
        let base_dir = get_base_dir(grep::GREP)?;

        // Validate path to prevent traversal attacks
        let validated_path =
            validate_path(path_str, &base_dir).map_err(|e| ToolError::ExecutionFailed {
                tool: grep::GREP.to_string(),
                source: Box::new(e),
            })?;

        // Collect matches
        let mut matches: Vec<GrepMatch> = Vec::new();
        let mut current_count = 0;

        if validated_path.is_file() {
            self.search_file(
                &validated_path,
                &pattern,
                max_results,
                &mut current_count,
                &mut matches,
            )
            .map_err(|e| ToolError::ExecutionFailed {
                tool: grep::GREP.to_string(),
                source: Box::new(e),
            })?;
        } else if validated_path.is_dir() {
            self.search_directory(
                &validated_path,
                &pattern,
                max_results,
                &mut current_count,
                &mut matches,
            )
            .map_err(|e| ToolError::ExecutionFailed {
                tool: grep::GREP.to_string(),
                source: Box::new(e),
            })?;
        } else {
            return Err(ToolError::ExecutionFailed {
                tool: grep::GREP.to_string(),
                source: Box::new(FileSystemError::NotFound {
                    path: validated_path,
                }),
            });
        }

        // Format and return results
        Ok(Self::format_results(&matches))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    // Use the shared mutex from lib.rs to serialize directory-changing tests

    #[test]
    fn test_grep_tool_name() {
        let tool = GrepTool::new();
        assert_eq!(tool.name(), grep::GREP);
    }

    #[test]
    fn test_grep_tool_description() {
        let tool = GrepTool::new();
        assert!(tool.description().contains(grep::GREP));
        assert!(tool.description().contains("pattern"));
    }

    #[test]
    fn test_grep_missing_pattern() {
        let tool = GrepTool::new();
        let args = HashMap::new();
        let result = tool.execute(&args);

        assert!(matches!(
            result,
            Err(ToolError::InvalidArguments { tool, reason })
            if tool == grep::GREP && reason.contains("pattern")
        ));
    }

    #[test]
    fn test_grep_missing_path() {
        let tool = GrepTool::new();
        let mut args = HashMap::new();
        args.insert("pattern".to_string(), "test".to_string());
        let result = tool.execute(&args);

        assert!(matches!(
            result,
            Err(ToolError::InvalidArguments { tool, reason })
            if tool == grep::GREP && reason.contains("path")
        ));
    }

    #[test]
    fn test_grep_invalid_regex() {
        let tool = GrepTool::new();
        let mut args = HashMap::new();
        args.insert("pattern".to_string(), "[invalid".to_string());
        args.insert("path".to_string(), ".".to_string());

        let result = tool.execute(&args);

        assert!(matches!(
            result,
            Err(ToolError::InvalidArguments { tool, reason })
            if tool == grep::GREP && reason.contains("Invalid regex")
        ));
    }

    #[test]
    fn test_grep_single_file() {
        // Serialize directory-changing tests to avoid race conditions
        let _guard = crate::DIR_MUTEX.lock().unwrap();

        let temp_dir = tempfile::tempdir().unwrap();
        let base_path = temp_dir.path();

        // Create a test file
        std::fs::write(
            base_path.join("test.txt"),
            "Hello World\nFoo Bar\nHello Again",
        )
        .unwrap();

        // Change to temp directory for the test
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(base_path).unwrap();

        let tool = GrepTool::new();
        let mut args = HashMap::new();
        args.insert("pattern".to_string(), "Hello".to_string());
        args.insert("path".to_string(), "test.txt".to_string());

        let result = tool.execute(&args).unwrap();

        // Restore original directory before temp_dir is dropped
        std::env::set_current_dir(original_dir).unwrap();

        assert!(result.contains("Hello World"));
        assert!(result.contains("Hello Again"));
        assert!(!result.contains("Foo Bar"));
    }

    #[test]
    fn test_grep_case_insensitive() {
        // Serialize directory-changing tests to avoid race conditions
        let _guard = crate::DIR_MUTEX.lock().unwrap();

        let temp_dir = tempfile::tempdir().unwrap();
        let base_path = temp_dir.path();

        std::fs::write(base_path.join("test.txt"), "HELLO World\nhello again").unwrap();

        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(base_path).unwrap();

        let tool = GrepTool::new();
        let mut args = HashMap::new();
        args.insert("pattern".to_string(), "hello".to_string());
        args.insert("path".to_string(), "test.txt".to_string());
        args.insert("case_insensitive".to_string(), "true".to_string());

        let result = tool.execute(&args).unwrap();

        // Restore original directory before temp_dir is dropped
        std::env::set_current_dir(original_dir).unwrap();

        assert!(result.contains("HELLO World"));
        assert!(result.contains("hello again"));
    }

    #[test]
    fn test_grep_no_matches() {
        // Serialize directory-changing tests to avoid race conditions
        let _guard = crate::DIR_MUTEX.lock().unwrap();

        let temp_dir = tempfile::tempdir().unwrap();
        let base_path = temp_dir.path();

        std::fs::write(base_path.join("test.txt"), "Hello World").unwrap();

        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(base_path).unwrap();

        let tool = GrepTool::new();
        let mut args = HashMap::new();
        args.insert("pattern".to_string(), "xyz".to_string());
        args.insert("path".to_string(), "test.txt".to_string());

        let result = tool.execute(&args).unwrap();

        // Restore original directory before temp_dir is dropped
        std::env::set_current_dir(original_dir).unwrap();

        assert_eq!(result, "No matches found");
    }

    #[test]
    fn test_grep_max_results() {
        // Serialize directory-changing tests to avoid race conditions
        let _guard = crate::DIR_MUTEX.lock().unwrap();

        let temp_dir = tempfile::tempdir().unwrap();
        let base_path = temp_dir.path();

        std::fs::write(
            base_path.join("test.txt"),
            "line1\nline2\nline3\nline4\nline5",
        )
        .unwrap();

        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(base_path).unwrap();

        let tool = GrepTool::new();
        let mut args = HashMap::new();
        args.insert("pattern".to_string(), "line".to_string());
        args.insert("path".to_string(), "test.txt".to_string());
        args.insert("max_results".to_string(), "3".to_string());

        let result = tool.execute(&args).unwrap();

        // Restore original directory before temp_dir is dropped
        std::env::set_current_dir(original_dir).unwrap();

        let lines: Vec<&str> = result.lines().collect();
        assert_eq!(lines.len(), 3);
    }

    #[test]
    fn test_grep_directory_search() {
        // Serialize directory-changing tests to avoid race conditions
        let _guard = crate::DIR_MUTEX.lock().unwrap();

        let temp_dir = tempfile::tempdir().unwrap();
        let base_path = temp_dir.path();

        std::fs::create_dir(base_path.join("subdir")).unwrap();
        std::fs::write(base_path.join("file1.txt"), "Hello from file1").unwrap();
        std::fs::write(base_path.join("subdir/file2.txt"), "Hello from file2").unwrap();

        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(base_path).unwrap();

        let tool = GrepTool::new();
        let mut args = HashMap::new();
        args.insert("pattern".to_string(), "Hello".to_string());
        args.insert("path".to_string(), ".".to_string());

        let result = tool.execute(&args).unwrap();

        // Restore original directory before temp_dir is dropped
        std::env::set_current_dir(original_dir).unwrap();

        assert!(result.contains("file1.txt"));
        assert!(result.contains("file2.txt"));
    }

    #[test]
    fn test_grep_path_traversal_blocked() {
        // Serialize directory-changing tests to avoid race conditions
        let _guard = crate::DIR_MUTEX.lock().unwrap();

        let temp_dir = tempfile::tempdir().unwrap();
        let base_path = temp_dir.path();

        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(base_path).unwrap();

        let tool = GrepTool::new();
        let mut args = HashMap::new();
        args.insert("pattern".to_string(), "test".to_string());
        args.insert("path".to_string(), "../../../etc/passwd".to_string());

        let result = tool.execute(&args);

        // Restore original directory before temp_dir is dropped
        std::env::set_current_dir(original_dir).unwrap();

        assert!(result.is_err());
    }
}
