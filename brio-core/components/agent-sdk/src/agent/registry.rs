//! Tool registry builder with preset configurations.
//!
//! This module provides [`ToolRegistryBuilder`] for creating pre-configured
//! tool registries tailored to different agent types.

use crate::tools::ToolRegistry;

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
    tools: ToolRegistry,
    flags: ToolRegistryFlags,
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
            tools: ToolRegistry::new(),
            flags: ToolRegistryFlags::DONE,
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
            tools: ToolRegistry::new(),
            flags: ToolRegistryFlags::DONE
                | ToolRegistryFlags::READ
                | ToolRegistryFlags::LIST
                | ToolRegistryFlags::GREP,
        }
    }

    /// Creates a file editor tool registry.
    ///
    /// Includes all read-only tools plus:
    /// - `write_file` - Write content to files
    #[must_use]
    pub fn file_editor() -> Self {
        Self {
            tools: ToolRegistry::new(),
            flags: ToolRegistryFlags::DONE
                | ToolRegistryFlags::READ
                | ToolRegistryFlags::LIST
                | ToolRegistryFlags::WRITE
                | ToolRegistryFlags::GREP,
        }
    }

    /// Creates a full-featured tool registry.
    ///
    /// Includes all file editor tools plus:
    /// - `shell` - Execute shell commands
    #[must_use]
    pub fn full() -> Self {
        Self {
            tools: ToolRegistry::new(),
            flags: ToolRegistryFlags::DONE
                | ToolRegistryFlags::READ
                | ToolRegistryFlags::LIST
                | ToolRegistryFlags::WRITE
                | ToolRegistryFlags::SHELL
                | ToolRegistryFlags::GREP,
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

    /// Builds the tool registry with the configured tools.
    ///
    /// # Note
    ///
    /// Currently, this returns an empty registry as the actual tool
    /// implementations are provided by the specific agent crates.
    /// The configuration flags are preserved for future use when
    /// the SDK provides built-in tool implementations.
    #[must_use]
    pub fn build(self) -> ToolRegistry {
        // In the future, this will register actual tool implementations
        // based on the configuration flags. For now, agents register
        // their own tools using the returned registry.
        self.tools
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
        // Currently returns empty registry
        assert!(registry.available_tools().is_empty());
    }
}
