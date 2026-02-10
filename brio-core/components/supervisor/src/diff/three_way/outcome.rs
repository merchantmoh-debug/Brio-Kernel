//! Outcome types for three-way merge operations.

use std::fmt::Write;
use std::path::PathBuf;
use thiserror::Error;

/// Errors that can occur during three-way merge operations.
#[derive(Debug, Error, Clone)]
pub enum ThreeWayMergeError {
    /// Invalid input (e.g., non-text files).
    #[error("Invalid input for three-way merge: {0}")]
    InvalidInput(String),

    /// Binary files cannot be merged using text-based algorithms.
    #[error("Cannot merge binary files: {0}")]
    BinaryFile(PathBuf),

    /// Maximum file size exceeded.
    #[error("File too large: {path} (max {max_size} bytes, got {actual_size})")]
    FileTooLarge {
        /// Path to the file.
        path: PathBuf,
        /// Maximum allowed size in bytes.
        max_size: usize,
        /// Actual file size in bytes.
        actual_size: usize,
    },
}

/// The result of a three-way merge operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MergeOutcome {
    /// The merge was successful with no conflicts.
    Merged(String),
    /// The merge has conflicts that need manual resolution.
    Conflicts(Vec<LineConflict>),
}

/// Represents a conflict at the line level.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineConflict {
    line_start: usize,
    pub(crate) line_end: usize,
    base_lines: Vec<String>,
    lines_a: Vec<String>,
    lines_b: Vec<String>,
}

impl LineConflict {
    /// Creates a new line conflict.
    #[must_use]
    pub fn new(
        line_start: usize,
        line_end: usize,
        base_lines: Vec<String>,
        lines_a: Vec<String>,
        lines_b: Vec<String>,
    ) -> Self {
        Self {
            line_start,
            line_end,
            base_lines,
            lines_a,
            lines_b,
        }
    }

    /// Returns the start line number in the merged file (1-based).
    #[must_use]
    pub const fn line_start(&self) -> usize {
        self.line_start
    }

    /// Returns the end line number in the merged file (1-based, exclusive).
    #[must_use]
    pub const fn line_end(&self) -> usize {
        self.line_end
    }

    /// Returns lines from the base version, if applicable.
    #[must_use]
    pub fn base_lines(&self) -> &[String] {
        &self.base_lines
    }

    /// Returns lines from branch A.
    #[must_use]
    pub fn branch_a_lines(&self) -> &[String] {
        &self.lines_a
    }

    /// Returns lines from branch B.
    #[must_use]
    pub fn branch_b_lines(&self) -> &[String] {
        &self.lines_b
    }

    /// Formats the conflict using Git-style conflict markers.
    #[must_use]
    pub fn format_with_markers(&self, name_a: &str, name_b: &str) -> String {
        let mut output = String::new();

        write!(output, "<<<<<<< {name_a}").unwrap();
        if self.lines_a.is_empty() {
            output.push('\n');
        } else {
            output.push('\n');
            for line in &self.lines_a {
                output.push_str(line);
                output.push('\n');
            }
        }

        if !self.base_lines.is_empty() {
            output.push_str("||||||| base\n");
            for line in &self.base_lines {
                output.push_str(line);
                output.push('\n');
            }
        }

        output.push_str("=======\n");

        if !self.lines_b.is_empty() {
            for line in &self.lines_b {
                output.push_str(line);
                output.push('\n');
            }
        }

        writeln!(output, ">>>>>>> {name_b}").unwrap();

        output
    }
}

/// Configuration for three-way merge operations.
#[derive(Debug, Clone)]
pub struct ThreeWayConfig {
    max_file_size: usize,
    allow_binary: bool,
    branch_a_name: String,
    branch_b_name: String,
}

impl ThreeWayConfig {
    /// Creates a new configuration for three-way merge operations.
    pub fn new(
        max_file_size: usize,
        allow_binary: bool,
        name_a: impl Into<String>,
        name_b: impl Into<String>,
    ) -> Self {
        Self {
            max_file_size,
            allow_binary,
            branch_a_name: name_a.into(),
            branch_b_name: name_b.into(),
        }
    }

    /// Returns the maximum file size in bytes (default: 10MB).
    #[must_use]
    pub const fn max_file_size(&self) -> usize {
        self.max_file_size
    }

    /// Returns whether to allow binary file merging.
    #[must_use]
    pub const fn allow_binary(&self) -> bool {
        self.allow_binary
    }

    /// Returns the branch A name for conflict markers.
    #[must_use]
    pub fn branch_a_name(&self) -> &str {
        &self.branch_a_name
    }

    /// Returns the branch B name for conflict markers.
    #[must_use]
    pub fn branch_b_name(&self) -> &str {
        &self.branch_b_name
    }
}

impl Default for ThreeWayConfig {
    fn default() -> Self {
        Self {
            max_file_size: 10 * 1024 * 1024,
            allow_binary: false,
            branch_a_name: "branch-a".to_string(),
            branch_b_name: "branch-b".to_string(),
        }
    }
}
