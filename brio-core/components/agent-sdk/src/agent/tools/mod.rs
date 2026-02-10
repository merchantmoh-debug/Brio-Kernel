//! Shared tool implementations for agents.
//!
//! This module provides ready-to-use tool implementations that can be
//! shared across different agent types. All tools include proper error
//! handling and security validation.
//!
//! # Available Tools
//!
//! - **Control Tools**: [`DoneTool`] - Mark task completion
//! - **File System Tools**: [`ReadFileTool`], [`WriteFileTool`], [`ListDirectoryTool`]
//! - **Shell Tools**: [`ShellTool`] - Execute shell commands with security
//! - **Branch Tools**: [`CreateBranchTool`], [`ListBranchesTool`] - Manage branches
//! - **Search Tools**: [`GrepTool`] - Search files with regex patterns
//!
//! # Example
//!
//! ```ignore
//! use agent_sdk::agent::tools::{DoneTool, ReadFileTool, WriteFileTool, ShellTool, GrepTool};
//! use agent_sdk::agent::parsers::{create_done_parser, create_read_parser, create_write_parser, create_shell_parser, create_grep_parser};
//! use agent_sdk::ToolRegistry;
//!
//! let mut registry = ToolRegistry::new();
//!
//! // Register done tool
//! registry.register("done", Box::new(DoneTool), create_done_parser());
//!
//! // Register file tools
//! registry.register("read_file", Box::new(ReadFileTool::new(1024 * 1024)), create_read_parser());
//! registry.register("write_file", Box::new(WriteFileTool), create_write_parser());
//!
//! // Register shell tool with allowlist
//! let allowlist = vec!["ls".to_string(), "cat".to_string()];
//! registry.register("shell", Box::new(ShellTool::new(allowlist)), create_shell_parser());
//!
//! // Register grep tool
//! registry.register("grep", Box::new(GrepTool::new()), create_grep_parser());
//! ```

pub mod branch;
pub mod control;
pub mod fs;
pub mod grep;
pub mod shell;

pub use branch::{
    BranchCreationCallback, BranchCreationConfig, BranchCreationResult, BranchId, BranchInfo,
    BranchToolError, CreateBranchTool, ListBranchesTool,
};
pub use control::DoneTool;
pub use fs::{ListDirectoryTool, ReadFileTool, WriteFileTool};
pub use grep::GrepTool;
pub use shell::ShellTool;
