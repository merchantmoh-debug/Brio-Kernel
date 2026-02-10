//! Grep Tool - Provides pattern matching capabilities.
//!
//! This tool component allows agents to search for patterns within files,
//! returning matches with line numbers and context.
//!
//! # Security Considerations
//!
//! - Paths are validated to prevent directory traversal attacks
//! - Pattern matching is performed safely without regex injection risks
//! - File access is limited to the working directory

// WIT bindings generate many undocumented items - this is expected for auto-generated code
#![allow(missing_docs)]

use std::fs::File;
use std::io::{self, BufRead, BufReader};
use wit_bindgen::generate;

// Generate WIT bindings
generate!({
    world: "grep-tool",
    path: "../../../wit",
    export_macro_name: "export_grep_tool",
});

export_grep_tool!(GrepTool);

/// Errors that can occur during grep operations.
#[derive(Debug, Clone)]
pub enum GrepError {
    /// The provided path is invalid or contains forbidden characters.
    InvalidPath(String),
    /// The specified file was not found.
    FileNotFound(String),
    /// Permission denied when accessing the file.
    PermissionDenied(String),
    /// An I/O error occurred while reading the file.
    IoError(String),
    /// The search pattern is invalid.
    InvalidPattern(String),
}

impl std::fmt::Display for GrepError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GrepError::InvalidPath(msg) => write!(f, "Invalid path: {msg}"),
            GrepError::FileNotFound(path) => write!(f, "File not found: {path}"),
            GrepError::PermissionDenied(path) => write!(f, "Permission denied: {path}"),
            GrepError::IoError(msg) => write!(f, "IO error: {msg}"),
            GrepError::InvalidPattern(msg) => write!(f, "Invalid pattern: {msg}"),
        }
    }
}

impl From<GrepError> for String {
    fn from(err: GrepError) -> Self {
        err.to_string()
    }
}

impl From<io::Error> for GrepError {
    fn from(err: io::Error) -> Self {
        match err.kind() {
            io::ErrorKind::NotFound => GrepError::FileNotFound(err.to_string()),
            io::ErrorKind::PermissionDenied => GrepError::PermissionDenied(err.to_string()),
            _ => GrepError::IoError(err.to_string()),
        }
    }
}

struct GrepTool;

impl GrepTool {
    fn validate_path(path: &str) -> Result<(), GrepError> {
        if path.contains('\0') {
            return Err(GrepError::InvalidPath(
                "Path contains null bytes".to_string(),
            ));
        }
        if path.contains("../") || path.contains("..\\") || path.starts_with("../") {
            return Err(GrepError::InvalidPath(
                "Path traversal not allowed".to_string(),
            ));
        }
        if path.starts_with('/') || (path.len() >= 2 && path[1..].starts_with(':')) {
            return Err(GrepError::InvalidPath(
                "Absolute paths not allowed".to_string(),
            ));
        }
        Ok(())
    }

    fn validate_pattern(pattern: &str) -> Result<(), GrepError> {
        if pattern.is_empty() {
            return Err(GrepError::InvalidPattern(
                "Pattern cannot be empty".to_string(),
            ));
        }
        if pattern.len() > 1000 {
            return Err(GrepError::InvalidPattern(
                "Pattern too long (max 1000 chars)".to_string(),
            ));
        }
        Ok(())
    }
}

impl exports::brio::core::tool_grep::Guest for GrepTool {
    fn grep(
        pattern: String,
        path: String,
    ) -> Result<Vec<exports::brio::core::tool_grep::GrepResult>, String> {
        GrepTool::validate_path(&path).map_err(|e| e.to_string())?;
        GrepTool::validate_pattern(&pattern).map_err(|e| e.to_string())?;

        let file = File::open(&path).map_err(|e| GrepError::from(e).to_string())?;
        let reader = BufReader::new(file);
        let mut matches = Vec::new();

        for (line_num, line) in reader.lines().enumerate() {
            let line = line.map_err(|e| GrepError::from(e).to_string())?;
            if line.contains(&pattern) {
                let line_number = u32::try_from(line_num + 1)
                    .map_err(|_| "Line number exceeds maximum supported value".to_string())?;
                matches.push(exports::brio::core::tool_grep::GrepMatch {
                    line_number,
                    content: line,
                });
            }
        }

        Ok(vec![exports::brio::core::tool_grep::GrepResult {
            file_path: path,
            matches,
        }])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_path_safe() {
        assert!(GrepTool::validate_path("file.txt").is_ok());
        assert!(GrepTool::validate_path("src/main.rs").is_ok());
    }

    #[test]
    fn test_validate_path_traversal() {
        assert!(GrepTool::validate_path("../etc/passwd").is_err());
        assert!(GrepTool::validate_path("safe/../unsafe").is_err());
    }

    #[test]
    fn test_validate_path_absolute() {
        assert!(GrepTool::validate_path("/etc/passwd").is_err());
    }

    #[test]
    fn test_validate_pattern_empty() {
        assert!(GrepTool::validate_pattern("").is_err());
    }
}
