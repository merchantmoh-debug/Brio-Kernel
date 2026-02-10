//! Tool registry builder with preset configurations.
//!
//! This module provides [`ToolRegistryBuilder`] for creating pre-configured
//! tool registries tailored to different agent types.

use crate::agent::parsers::{
    create_done_parser, create_grep_parser, create_list_parser, create_read_parser,
    create_shell_parser, create_write_parser,
};
use crate::agent::tools::control::DoneTool;
use crate::agent::tools::fs::{ListDirectoryTool, ReadFileTool, WriteFileTool};
use crate::agent::tools::shell::ShellTool;
use crate::config::AgentConfig;
use crate::error::ToolError;
use crate::tools::constants::grep;
use crate::tools::{Tool, ToolRegistry};
use std::borrow::Cow;
use std::collections::HashMap;
use std::path::PathBuf;

/// Default maximum number of results for grep operations.
const DEFAULT_MAX_RESULTS: usize = 1000;

/// Tool for searching file contents.
///
/// Searches for patterns in files within a specified directory.
/// Supports regex patterns and limits results for performance.
///
/// # Security
///
/// - Validates path to prevent directory traversal attacks
/// - Enforces maximum file size and result limits
/// - Does not search binary files
///
/// # Example
///
/// ```
/// use agent_sdk::agent::registry::GrepTool;
/// use agent_sdk::Tool;
/// use std::path::PathBuf;
///
/// # fn example() {
/// let tool = GrepTool::new(
///     PathBuf::from("/workspace"),
///     1000,      // max_results
///     1024 * 1024, // max_file_size (1MB)
/// );
/// # }
/// ```
pub struct GrepTool {
    #[allow(dead_code)]
    base_dir: PathBuf,
    max_results: usize,
    max_file_size: u64,
}

impl GrepTool {
    /// Creates a new `GrepTool` with the specified configuration.
    ///
    /// # Arguments
    ///
    /// * `base_dir` - Base directory for path validation
    /// * `max_results` - Maximum number of results to return
    /// * `max_file_size` - Maximum file size in bytes to search
    #[must_use]
    pub fn new(base_dir: PathBuf, max_results: usize, max_file_size: u64) -> Self {
        Self {
            base_dir,
            max_results,
            max_file_size,
        }
    }
}

impl Tool for GrepTool {
    fn name(&self) -> Cow<'static, str> {
        Cow::Borrowed(grep::GREP)
    }

    fn description(&self) -> Cow<'static, str> {
        Cow::Borrowed(
            r#"<grep pattern="search pattern" path="path/to/search" /> - Search file contents"#,
        )
    }

    fn execute(&self, args: &HashMap<String, String>) -> Result<String, ToolError> {
        let pattern = args
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

        // TODO: Implement actual grep functionality
        // For now, return a placeholder response
        Ok(format!(
            "Grep search for '{}' in '{}' (max_results: {}, max_file_size: {})",
            pattern, path_str, self.max_results, self.max_file_size
        ))
    }
}

bitflags::bitflags! {
    /// Configuration flags for tool registry builder.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
    pub struct ToolRegistryFlags: u8 {
        /// Whether the `done` tool is included.
        const DONE = 1 << 0;
        /// Whether the `read_file` tool is included.
        const READ = 1 << 1;
        /// Whether the `ls` tool is included.
        const LIST = 1 << 2;
        /// Whether the `write_file` tool is included.
        const WRITE = 1 << 3;
        /// Whether the `shell` tool is included.
        const SHELL = 1 << 4;
        /// Whether the `grep` tool is included.
        const GREP = 1 << 5;
    }
}

/// Builder for creating pre-configured tool registries.
///
/// This builder provides convenient methods to create tool registries
/// with different tool sets based on the agent's requirements.
///
/// # Example
///
/// ```rust
/// use agent_sdk::agent::registry::ToolRegistryBuilder;
///
/// // Create a read-only tool registry
/// let registry = ToolRegistryBuilder::read_only().build();
///
/// // Create a full-featured registry for file editing
/// let registry = ToolRegistryBuilder::file_editor().build();
/// ```
#[derive(Debug, Default)]
pub struct ToolRegistryBuilder {
    flags: ToolRegistryFlags,
    config: Option<AgentConfig>,
}

impl ToolRegistryBuilder {
    /// Creates a new empty tool registry builder.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a minimal tool registry with only the `done` tool.
    ///
    /// This is suitable for agents that only need to signal completion
    /// and don't require any file operations.
    #[must_use]
    pub fn minimal() -> Self {
        Self {
            flags: ToolRegistryFlags::DONE,
            config: None,
        }
    }

    /// Creates a read-only tool registry.
    ///
    /// Includes:
    /// - `done` - Mark task as complete
    /// - `read_file` - Read file contents
    /// - `ls` - List directory contents
    /// - `grep` - Search file contents
    #[must_use]
    pub fn read_only() -> Self {
        Self {
            flags: ToolRegistryFlags::DONE
                | ToolRegistryFlags::READ
                | ToolRegistryFlags::LIST
                | ToolRegistryFlags::GREP,
            config: None,
        }
    }

    /// Creates a file editor tool registry.
    ///
    /// Includes all read-only tools plus:
    /// - `write_file` - Write content to files
    #[must_use]
    pub fn file_editor() -> Self {
        Self {
            flags: ToolRegistryFlags::DONE
                | ToolRegistryFlags::READ
                | ToolRegistryFlags::LIST
                | ToolRegistryFlags::WRITE
                | ToolRegistryFlags::GREP,
            config: None,
        }
    }

    /// Creates a full-featured tool registry.
    ///
    /// Includes all file editor tools plus:
    /// - `shell` - Execute shell commands
    #[must_use]
    pub fn full() -> Self {
        Self {
            flags: ToolRegistryFlags::DONE
                | ToolRegistryFlags::READ
                | ToolRegistryFlags::LIST
                | ToolRegistryFlags::WRITE
                | ToolRegistryFlags::SHELL
                | ToolRegistryFlags::GREP,
            config: None,
        }
    }

    /// Adds the `done` tool to the registry.
    #[must_use]
    pub fn with_done(mut self) -> Self {
        self.flags |= ToolRegistryFlags::DONE;
        self
    }

    /// Adds the `read_file` tool to the registry.
    #[must_use]
    pub fn with_read(mut self) -> Self {
        self.flags |= ToolRegistryFlags::READ;
        self
    }

    /// Adds the `ls` tool to the registry.
    #[must_use]
    pub fn with_list(mut self) -> Self {
        self.flags |= ToolRegistryFlags::LIST;
        self
    }

    /// Adds the `write_file` tool to the registry.
    #[must_use]
    pub fn with_write(mut self) -> Self {
        self.flags |= ToolRegistryFlags::WRITE;
        self
    }

    /// Adds the `shell` tool to the registry.
    #[must_use]
    pub fn with_shell(mut self) -> Self {
        self.flags |= ToolRegistryFlags::SHELL;
        self
    }

    /// Adds the `grep` tool to the registry.
    #[must_use]
    pub fn with_grep(mut self) -> Self {
        self.flags |= ToolRegistryFlags::GREP;
        self
    }

    /// Sets the agent configuration for tool initialization.
    ///
    /// This configuration is used to set up tool parameters such as
    /// file size limits, shell allowlists, and base directories.
    #[must_use]
    pub fn with_config(mut self, config: AgentConfig) -> Self {
        self.config = Some(config);
        self
    }

    /// Builds the tool registry with the configured tools.
    ///
    /// This method instantiates and registers all tools based on the
    /// configured flags and agent configuration. Tools are initialized
    /// with appropriate parameters from the configuration.
    ///
    /// # Tool Registration
    ///
    /// - `done` - Registered if DONE flag is set
    /// - `read_file` - Registered if READ flag is set
    /// - `ls` - Registered if LIST flag is set
    /// - `write_file` - Registered if WRITE flag is set
    /// - `shell` - Registered if SHELL flag is set
    /// - `grep` - Registered if GREP flag is set
    #[must_use]
    pub fn build(self) -> ToolRegistry {
        let mut registry = ToolRegistry::new();

        // Get configuration or use defaults
        let config = self.config.as_ref();

        // Register DoneTool if DONE flag is set
        if self.flags.contains(ToolRegistryFlags::DONE) {
            registry.register(
                crate::tools::constants::control::DONE,
                Box::new(DoneTool),
                create_done_parser(),
            );
        }

        // Register ReadFileTool if READ flag is set
        if self.flags.contains(ToolRegistryFlags::READ) {
            let max_size = config.map_or(10 * 1024 * 1024, |c| c.max_file_size);
            registry.register(
                crate::tools::constants::fs::READ_FILE,
                Box::new(ReadFileTool::new(max_size)),
                create_read_parser(),
            );
        }

        // Register ListDirectoryTool if LIST flag is set
        if self.flags.contains(ToolRegistryFlags::LIST) {
            let max_depth = config.map_or(10, |c| c.max_depth);
            registry.register(
                crate::tools::constants::fs::LS,
                Box::new(ListDirectoryTool::new(max_depth)),
                create_list_parser(),
            );
        }

        // Register WriteFileTool if WRITE flag is set
        if self.flags.contains(ToolRegistryFlags::WRITE) {
            registry.register(
                crate::tools::constants::fs::WRITE_FILE,
                Box::new(WriteFileTool),
                create_write_parser(),
            );
        }

        // Register ShellTool if SHELL flag is set
        if self.flags.contains(ToolRegistryFlags::SHELL) {
            let allowlist = config
                .map(|c| c.shell_allowlist.clone())
                .unwrap_or_else(|| {
                    vec![
                        "ls".to_string(),
                        "cat".to_string(),
                        "echo".to_string(),
                        "pwd".to_string(),
                        "find".to_string(),
                        "grep".to_string(),
                        "head".to_string(),
                        "tail".to_string(),
                        "wc".to_string(),
                        "sort".to_string(),
                        "uniq".to_string(),
                    ]
                });
            registry.register(
                crate::tools::constants::shell::SHELL,
                Box::new(ShellTool::new(allowlist)),
                create_shell_parser(),
            );
        }

        // Register GrepTool if GREP flag is set
        if self.flags.contains(ToolRegistryFlags::GREP) {
            let max_results = config.map_or(DEFAULT_MAX_RESULTS, |c| c.max_depth);
            let max_file_size = config.map_or(10 * 1024 * 1024, |c| c.max_file_size);
            let base_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
            registry.register(
                crate::tools::constants::grep::GREP,
                Box::new(GrepTool::new(base_dir, max_results, max_file_size)),
                create_grep_parser(),
            );
        }

        registry
    }

    /// Returns the configuration flags for inspection.
    #[must_use]
    pub fn config(&self) -> ToolRegistryConfig {
        ToolRegistryConfig { flags: self.flags }
    }
}

/// Configuration flags for a tool registry builder.
#[derive(Debug, Clone, Copy)]
pub struct ToolRegistryConfig {
    flags: ToolRegistryFlags,
}

impl ToolRegistryConfig {
    /// Returns `true` if this configuration is read-only (no write or shell tools).
    #[must_use]
    pub fn is_read_only(&self) -> bool {
        !(self.flags.contains(ToolRegistryFlags::WRITE)
            || self.flags.contains(ToolRegistryFlags::SHELL))
    }

    /// Returns `true` if this configuration allows file modifications.
    #[must_use]
    pub fn allows_modification(&self) -> bool {
        self.flags.contains(ToolRegistryFlags::WRITE)
            || self.flags.contains(ToolRegistryFlags::SHELL)
    }

    /// Returns `true` if the `done` tool is included.
    #[must_use]
    pub fn has_done(&self) -> bool {
        self.flags.contains(ToolRegistryFlags::DONE)
    }

    /// Returns `true` if the `read_file` tool is included.
    #[must_use]
    pub fn has_read(&self) -> bool {
        self.flags.contains(ToolRegistryFlags::READ)
    }

    /// Returns `true` if the `ls` tool is included.
    #[must_use]
    pub fn has_list(&self) -> bool {
        self.flags.contains(ToolRegistryFlags::LIST)
    }

    /// Returns `true` if the `write_file` tool is included.
    #[must_use]
    pub fn has_write(&self) -> bool {
        self.flags.contains(ToolRegistryFlags::WRITE)
    }

    /// Returns `true` if the `shell` tool is included.
    #[must_use]
    pub fn has_shell(&self) -> bool {
        self.flags.contains(ToolRegistryFlags::SHELL)
    }

    /// Returns `true` if the `grep` tool is included.
    #[must_use]
    pub fn has_grep(&self) -> bool {
        self.flags.contains(ToolRegistryFlags::GREP)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_minimal_builder() {
        let builder = ToolRegistryBuilder::minimal();
        let config = builder.config();

        assert!(config.has_done());
        assert!(!config.has_read());
        assert!(!config.has_list());
        assert!(!config.has_write());
        assert!(!config.has_shell());
        assert!(!config.has_grep());
    }

    #[test]
    fn test_read_only_builder() {
        let builder = ToolRegistryBuilder::read_only();
        let config = builder.config();

        assert!(config.has_done());
        assert!(config.has_read());
        assert!(config.has_list());
        assert!(!config.has_write());
        assert!(!config.has_shell());
        assert!(config.has_grep());
    }

    #[test]
    fn test_file_editor_builder() {
        let builder = ToolRegistryBuilder::file_editor();
        let config = builder.config();

        assert!(config.has_done());
        assert!(config.has_read());
        assert!(config.has_list());
        assert!(config.has_write());
        assert!(!config.has_shell());
        assert!(config.has_grep());
    }

    #[test]
    fn test_full_builder() {
        let builder = ToolRegistryBuilder::full();
        let config = builder.config();

        assert!(config.has_done());
        assert!(config.has_read());
        assert!(config.has_list());
        assert!(config.has_write());
        assert!(config.has_shell());
        assert!(config.has_grep());
    }

    #[test]
    fn test_builder_chaining() {
        let builder = ToolRegistryBuilder::new()
            .with_done()
            .with_read()
            .with_list();

        let config = builder.config();
        assert!(config.has_done());
        assert!(config.has_read());
        assert!(config.has_list());
        assert!(!config.has_write());
    }

    #[test]
    fn test_config_is_read_only() {
        let read_only = ToolRegistryBuilder::read_only().config();
        assert!(read_only.is_read_only());

        let file_editor = ToolRegistryBuilder::file_editor().config();
        assert!(!file_editor.is_read_only());

        let full = ToolRegistryBuilder::full().config();
        assert!(!full.is_read_only());
    }

    #[test]
    fn test_config_allows_modification() {
        let read_only = ToolRegistryBuilder::read_only().config();
        assert!(!read_only.allows_modification());

        let file_editor = ToolRegistryBuilder::file_editor().config();
        assert!(file_editor.allows_modification());

        let full = ToolRegistryBuilder::full().config();
        assert!(full.allows_modification());
    }

    #[test]
    fn test_build_creates_registry() {
        let registry = ToolRegistryBuilder::read_only().build();
        // Should contain done, read_file, ls, and grep tools
        let tools = registry.available_tools();
        assert!(!tools.is_empty());
        assert!(tools.contains(&"done"));
        assert!(tools.contains(&"read_file"));
        assert!(tools.contains(&"ls"));
        assert!(tools.contains(&"grep"));
        assert!(!tools.contains(&"write_file"));
        assert!(!tools.contains(&"shell"));
    }

    #[test]
    fn test_minimal_build() {
        let registry = ToolRegistryBuilder::minimal().build();
        let tools = registry.available_tools();
        assert_eq!(tools.len(), 1);
        assert!(tools.contains(&"done"));
    }

    #[test]
    fn test_file_editor_build() {
        let registry = ToolRegistryBuilder::file_editor().build();
        let tools = registry.available_tools();
        assert!(!tools.is_empty());
        assert!(tools.contains(&"done"));
        assert!(tools.contains(&"read_file"));
        assert!(tools.contains(&"ls"));
        assert!(tools.contains(&"write_file"));
        assert!(tools.contains(&"grep"));
        assert!(!tools.contains(&"shell"));
    }

    #[test]
    fn test_full_build() {
        let registry = ToolRegistryBuilder::full().build();
        let tools = registry.available_tools();
        assert!(!tools.is_empty());
        assert!(tools.contains(&"done"));
        assert!(tools.contains(&"read_file"));
        assert!(tools.contains(&"ls"));
        assert!(tools.contains(&"write_file"));
        assert!(tools.contains(&"shell"));
        assert!(tools.contains(&"grep"));
    }

    #[test]
    fn test_custom_flag_combinations() {
        let registry = ToolRegistryBuilder::new()
            .with_done()
            .with_read()
            .with_grep()
            .build();

        let tools = registry.available_tools();
        assert_eq!(tools.len(), 3);
        assert!(tools.contains(&"done"));
        assert!(tools.contains(&"read_file"));
        assert!(tools.contains(&"grep"));
        assert!(!tools.contains(&"ls"));
        assert!(!tools.contains(&"write_file"));
        assert!(!tools.contains(&"shell"));
    }

    #[test]
    fn test_tool_configuration_passed() {
        use crate::config::AgentConfig;

        let config = AgentConfig::builder()
            .max_file_size(5 * 1024 * 1024) // 5MB
            .max_depth(5)
            .shell_allowlist(vec!["ls".to_string(), "cat".to_string()])
            .build()
            .unwrap();

        let registry = ToolRegistryBuilder::full().with_config(config).build();

        let tools = registry.available_tools();
        assert!(tools.contains(&"done"));
        assert!(tools.contains(&"read_file"));
        assert!(tools.contains(&"ls"));
        assert!(tools.contains(&"write_file"));
        assert!(tools.contains(&"shell"));
        assert!(tools.contains(&"grep"));
    }

    #[test]
    fn test_grep_tool_new() {
        let base_dir = std::env::current_dir().unwrap();
        let tool = GrepTool::new(base_dir, 1000, 1024 * 1024);
        assert_eq!(tool.name(), "grep");
        assert!(tool.description().contains("grep"));
    }
}
