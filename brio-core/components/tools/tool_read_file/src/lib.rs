//! Read File Tool - Provides file reading capabilities.
//!
//! This tool component allows agents to read file contents, including
//! reading entire files or specific line ranges.
//!
//! # Security Considerations
//!
//! - Paths are validated to prevent directory traversal attacks
//! - Absolute paths are rejected
//! - Large files are handled with line limits
//! - File size limits prevent `DoS` attacks

// WIT bindings generate many undocumented items - this is expected for auto-generated code
#![allow(missing_docs)]

use std::fs::File;
use std::io::{self, BufRead, BufReader, Read};
use wit_bindgen::generate;

generate!({
    world: "read-file-tool",
    path: "../../../wit",
    export_macro_name: "export_read_file_tool",
});

export_read_file_tool!(ReadFileTool);

const MAX_FILE_SIZE: u64 = 10 * 1024 * 1024;
const MAX_LINES: u32 = 10_000;

#[derive(Debug, Clone)]
pub enum ReadFileError {
    InvalidPath(String),
    FileNotFound(String),
    PermissionDenied(String),
    IoError(String),
    FileTooLarge(u64),
    InvalidLineRange { start: u32, end: u32 },
    LineLimitExceeded(u32),
}

impl std::fmt::Display for ReadFileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReadFileError::InvalidPath(msg) => write!(f, "Invalid path: {msg}"),
            ReadFileError::FileNotFound(path) => write!(f, "File not found: {path}"),
            ReadFileError::PermissionDenied(path) => write!(f, "Permission denied: {path}"),
            ReadFileError::IoError(msg) => write!(f, "IO error: {msg}"),
            ReadFileError::FileTooLarge(size) => {
                write!(f, "File too large: {size} bytes (max {MAX_FILE_SIZE})")
            }
            ReadFileError::InvalidLineRange { start, end } => {
                write!(f, "Invalid line range: {start}-{end}")
            }
            ReadFileError::LineLimitExceeded(count) => {
                write!(f, "Line count {count} exceeds maximum {MAX_LINES}")
            }
        }
    }
}

impl From<ReadFileError> for String {
    fn from(err: ReadFileError) -> Self {
        err.to_string()
    }
}

impl From<io::Error> for ReadFileError {
    fn from(err: io::Error) -> Self {
        match err.kind() {
            io::ErrorKind::NotFound => ReadFileError::FileNotFound(err.to_string()),
            io::ErrorKind::PermissionDenied => ReadFileError::PermissionDenied(err.to_string()),
            _ => ReadFileError::IoError(err.to_string()),
        }
    }
}

struct ReadFileTool;

impl ReadFileTool {
    fn validate_path(path: &str) -> Result<(), ReadFileError> {
        if path.contains('\0') {
            return Err(ReadFileError::InvalidPath(
                "Path contains null bytes".to_string(),
            ));
        }
        if path.contains("../") || path.contains("..\\") || path.starts_with("../") {
            return Err(ReadFileError::InvalidPath(
                "Path traversal not allowed".to_string(),
            ));
        }
        if path.starts_with('/') || (path.len() >= 2 && path[1..].starts_with(':')) {
            return Err(ReadFileError::InvalidPath(
                "Absolute paths not allowed".to_string(),
            ));
        }
        Ok(())
    }

    fn check_file_size(path: &str) -> Result<u64, ReadFileError> {
        let metadata = std::fs::metadata(path).map_err(ReadFileError::from)?;
        let size = metadata.len();
        if size > MAX_FILE_SIZE {
            return Err(ReadFileError::FileTooLarge(size));
        }
        Ok(size)
    }

    fn read_file_internal(path: &str) -> Result<String, ReadFileError> {
        Self::validate_path(path)?;
        Self::check_file_size(path)?;
        let mut file = File::open(path).map_err(ReadFileError::from)?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)
            .map_err(ReadFileError::from)?;
        Ok(contents)
    }

    fn read_file_range_internal(
        path: &str,
        start_line: u32,
        end_line: u32,
    ) -> Result<String, ReadFileError> {
        if start_line == 0 || end_line < start_line {
            return Err(ReadFileError::InvalidLineRange {
                start: start_line,
                end: end_line,
            });
        }
        let line_count = end_line - start_line + 1;
        if line_count > MAX_LINES {
            return Err(ReadFileError::LineLimitExceeded(line_count));
        }
        Self::validate_path(path)?;
        Self::check_file_size(path)?;
        let file = File::open(path).map_err(ReadFileError::from)?;
        let reader = BufReader::new(file);
        let mut result = String::new();
        for (line_num, line) in reader.lines().enumerate() {
            let current_line = u32::try_from(line_num + 1).map_err(|_| {
                ReadFileError::IoError("Line number exceeds maximum supported value".to_string())
            })?;
            if current_line > end_line {
                break;
            }
            if current_line >= start_line {
                let line = line.map_err(ReadFileError::from)?;
                result.push_str(&line);
                result.push('\n');
            }
        }
        Ok(result)
    }
}

impl exports::brio::core::tool_read_file::Guest for ReadFileTool {
    fn read_file(path: String) -> Result<String, String> {
        Self::read_file_internal(&path).map_err(std::convert::Into::into)
    }

    fn read_file_range(path: String, start_line: u32, end_line: u32) -> Result<String, String> {
        Self::read_file_range_internal(&path, start_line, end_line)
            .map_err(std::convert::Into::into)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_path_safe() {
        assert!(ReadFileTool::validate_path("file.txt").is_ok());
        assert!(ReadFileTool::validate_path("src/main.rs").is_ok());
    }

    #[test]
    fn test_validate_path_traversal() {
        assert!(ReadFileTool::validate_path("../etc/passwd").is_err());
        assert!(ReadFileTool::validate_path("safe/../unsafe").is_err());
    }

    #[test]
    fn test_validate_path_absolute() {
        assert!(ReadFileTool::validate_path("/etc/passwd").is_err());
    }
}
