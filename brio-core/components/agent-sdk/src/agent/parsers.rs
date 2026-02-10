//! Parser functions for tool invocations.
//!
//! This module provides pre-compiled regex parsers for extracting tool
//! invocations from agent responses. All parsers use `std::sync::LazyLock`
//! to ensure regex compilation happens only once at first use.

use crate::tools::ToolParser;
use regex::Captures;
use std::collections::HashMap;
use std::sync::{Arc, LazyLock};

/// Parser for the done tool.
///
/// Matches: `<done>summary text</done>`
static DONE_PARSER: LazyLock<Arc<ToolParser>> = LazyLock::new(|| {
    Arc::new(ToolParser::new_unchecked(
        r"<done>\s*(.*?)\s*</done>",
        |caps: &Captures| {
            let mut args = HashMap::new();
            if let Some(m) = caps.get(1) {
                args.insert("summary".to_string(), m.as_str().to_string());
            }
            args
        },
    ))
});

/// Parser for the `read_file` tool.
///
/// Matches: `<read_file path="path/to/file" />` or `<read_file path="path/to/file"/>`
static READ_PARSER: LazyLock<Arc<ToolParser>> = LazyLock::new(|| {
    Arc::new(ToolParser::new_unchecked(
        r#"<read_file\s+path="([^"]+)"\s*/?>"#,
        |caps: &Captures| {
            let mut args = HashMap::new();
            if let Some(m) = caps.get(1) {
                args.insert("path".to_string(), m.as_str().to_string());
            }
            args
        },
    ))
});

/// Parser for the ls (list directory) tool.
///
/// Matches: `<ls path="path/to/directory" />` or `<ls path="path/to/directory"/>`
static LIST_PARSER: LazyLock<Arc<ToolParser>> = LazyLock::new(|| {
    Arc::new(ToolParser::new_unchecked(
        r#"<ls\s+path="([^"]+)"\s*/?>"#,
        |caps: &Captures| {
            let mut args = HashMap::new();
            if let Some(m) = caps.get(1) {
                args.insert("path".to_string(), m.as_str().to_string());
            }
            args
        },
    ))
});

/// Parser for the `write_file` tool.
///
/// Matches: `<write_file path="path/to/file">content</write_file>`
static WRITE_PARSER: LazyLock<Arc<ToolParser>> = LazyLock::new(|| {
    Arc::new(ToolParser::new_unchecked(
        r#"<write_file\s+path="([^"]+)">\s*(.*?)\s*</write_file>"#,
        |caps: &Captures| {
            let mut args = HashMap::new();
            if let Some(m) = caps.get(1) {
                args.insert("path".to_string(), m.as_str().to_string());
            }
            if let Some(m) = caps.get(2) {
                args.insert("content".to_string(), m.as_str().to_string());
            }
            args
        },
    ))
});

/// Parser for the shell tool.
///
/// Matches: `<shell>command to execute</shell>`
static SHELL_PARSER: LazyLock<Arc<ToolParser>> = LazyLock::new(|| {
    Arc::new(ToolParser::new_unchecked(
        r"<shell>\s*(.*?)\s*</shell>",
        |caps: &Captures| {
            let mut args = HashMap::new();
            if let Some(m) = caps.get(1) {
                args.insert("command".to_string(), m.as_str().to_string());
            }
            args
        },
    ))
});

/// Parser for the grep tool.
///
/// Matches: `<grep pattern="search pattern" path="path/to/search" />`
static GREP_PARSER: LazyLock<Arc<ToolParser>> = LazyLock::new(|| {
    Arc::new(ToolParser::new_unchecked(
        r#"<grep\s+pattern="([^"]+)"(?:\s+path="([^"]*)")?\s*/?>"#,
        |caps: &Captures| {
            let mut args = HashMap::new();
            if let Some(m) = caps.get(1) {
                args.insert("pattern".to_string(), m.as_str().to_string());
            }
            if let Some(m) = caps.get(2)
                && !m.as_str().is_empty()
            {
                args.insert("path".to_string(), m.as_str().to_string());
            }
            args
        },
    ))
});

/// Parser for the `create_branch` tool.
///
/// Matches: `<create_branch name="branch-name" [parent="parent-id"] [inherit_config="true"] />`
static CREATE_BRANCH_PARSER: LazyLock<Arc<ToolParser>> = LazyLock::new(|| {
    Arc::new(ToolParser::new_unchecked(
        r#"<create_branch\s+name="([^"]+)"(?:\s+parent="([^"]*)")?(?:\s+inherit_config="([^"]*)")?\s*/?>"#,
        |caps: &Captures| {
            let mut args = HashMap::new();
            if let Some(m) = caps.get(1) {
                args.insert("name".to_string(), m.as_str().to_string());
            }
            if let Some(m) = caps.get(2)
                && !m.as_str().is_empty()
            {
                args.insert("parent".to_string(), m.as_str().to_string());
            }
            if let Some(m) = caps.get(3)
                && !m.as_str().is_empty()
            {
                args.insert("inherit_config".to_string(), m.as_str().to_string());
            }
            args
        },
    ))
});

/// Parser for the `list_branches` tool.
///
/// Matches: `<list_branches />` or `<list_branches/>`
static LIST_BRANCHES_PARSER: LazyLock<Arc<ToolParser>> = LazyLock::new(|| {
    Arc::new(ToolParser::new_unchecked(
        r"<list_branches\s*/?>",
        |_caps: &Captures| HashMap::new(),
    ))
});

/// Returns a clone of the done tool parser.
///
/// This parser extracts the summary text from `<done>` tags.
#[must_use]
pub fn create_done_parser() -> Arc<ToolParser> {
    Arc::clone(&DONE_PARSER)
}

/// Returns a clone of the `read_file` tool parser.
///
/// This parser extracts the `path` attribute from `<read_file>` tags.
#[must_use]
pub fn create_read_parser() -> Arc<ToolParser> {
    Arc::clone(&READ_PARSER)
}

/// Returns a clone of the ls (list directory) tool parser.
///
/// This parser extracts the `path` attribute from `<ls>` tags.
#[must_use]
pub fn create_list_parser() -> Arc<ToolParser> {
    Arc::clone(&LIST_PARSER)
}

/// Returns a clone of the `write_file` tool parser.
///
/// This parser extracts both the `path` attribute and the content
/// from `<write_file>` tags.
#[must_use]
pub fn create_write_parser() -> Arc<ToolParser> {
    Arc::clone(&WRITE_PARSER)
}

/// Returns a clone of the shell tool parser.
///
/// This parser extracts the command to execute from `<shell>` tags.
#[must_use]
pub fn create_shell_parser() -> Arc<ToolParser> {
    Arc::clone(&SHELL_PARSER)
}

/// Returns a clone of the grep tool parser.
///
/// This parser extracts the pattern and optional path from `<grep>` tags.
#[must_use]
pub fn create_grep_parser() -> Arc<ToolParser> {
    Arc::clone(&GREP_PARSER)
}

/// Returns a clone of the `create_branch` tool parser.
///
/// This parser extracts branch creation arguments from `<create_branch>` tags.
#[must_use]
pub fn create_create_branch_parser() -> Arc<ToolParser> {
    Arc::clone(&CREATE_BRANCH_PARSER)
}

/// Returns a clone of the `list_branches` tool parser.
///
/// This parser matches `<list_branches />` tags.
#[must_use]
pub fn create_list_branches_parser() -> Arc<ToolParser> {
    Arc::clone(&LIST_BRANCHES_PARSER)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_done_parser() {
        let parser = create_done_parser();
        let input = "<done>Task completed successfully</done>";
        let results = parser.parse(input);

        assert_eq!(results.len(), 1);
        // Note: Tool name is assigned by registry, parser extracts args only
        assert_eq!(
            results[0].args.get("summary"),
            Some(&"Task completed successfully".to_string())
        );
    }

    #[test]
    fn test_read_parser() {
        let parser = create_read_parser();
        let input = r#"<read_file path="src/main.rs" />"#;
        let results = parser.parse(input);

        assert_eq!(results.len(), 1);
        // Note: Tool name is assigned by registry, parser extracts args only
        assert_eq!(
            results[0].args.get("path"),
            Some(&"src/main.rs".to_string())
        );
    }

    #[test]
    fn test_list_parser() {
        let parser = create_list_parser();
        let input = r#"<ls path="src/"/>"#;
        let results = parser.parse(input);

        assert_eq!(results.len(), 1);
        // Note: Tool name is assigned by registry, parser extracts args only
        assert_eq!(results[0].args.get("path"), Some(&"src/".to_string()));
    }

    #[test]
    fn test_write_parser() {
        let parser = create_write_parser();
        let input = r#"<write_file path="test.txt">Hello, World!</write_file>"#;
        let results = parser.parse(input);

        assert_eq!(results.len(), 1);
        // Note: Tool name is assigned by registry, parser extracts args only
        assert_eq!(results[0].args.get("path"), Some(&"test.txt".to_string()));
        assert_eq!(
            results[0].args.get("content"),
            Some(&"Hello, World!".to_string())
        );
    }

    #[test]
    fn test_shell_parser() {
        let parser = create_shell_parser();
        let input = "<shell>ls -la</shell>";
        let results = parser.parse(input);

        assert_eq!(results.len(), 1);
        // Note: Tool name is assigned by registry, parser extracts args only
        assert_eq!(results[0].args.get("command"), Some(&"ls -la".to_string()));
    }

    #[test]
    fn test_grep_parser() {
        let parser = create_grep_parser();
        let input = r#"<grep pattern="fn main" path="src/" />"#;
        let results = parser.parse(input);

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].args.get("pattern"), Some(&"fn main".to_string()));
        assert_eq!(results[0].args.get("path"), Some(&"src/".to_string()));

        // Test without path attribute
        let input2 = r#"<grep pattern="test" />"#;
        let results2 = parser.parse(input2);

        assert_eq!(results2.len(), 1);
        assert_eq!(results2[0].args.get("pattern"), Some(&"test".to_string()));
        assert_eq!(results2[0].args.get("path"), None);
    }

    #[test]
    fn test_multiple_invocations() {
        let parser = create_read_parser();
        let input = r#"<read_file path="file1.rs" /><read_file path="file2.rs" />"#;
        let results = parser.parse(input);

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].args.get("path"), Some(&"file1.rs".to_string()));
        assert_eq!(results[1].args.get("path"), Some(&"file2.rs".to_string()));
    }
}
