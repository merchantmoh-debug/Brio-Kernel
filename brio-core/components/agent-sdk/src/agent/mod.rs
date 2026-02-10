//! Shared agent framework for standard AI-loop agents.
//!
//! This module provides the foundation for building standard agents with
//! the [`StandardAgent`] trait and supporting components.
//!
//! # Modules
//!
//! - **base**: Core traits and functions for standard agents
//! - **parsers**: Pre-compiled tool invocation parsers
//! - **registry**: Tool registry builder with preset configurations
//! - **tools**: Ready-to-use tool implementations

pub mod base;
pub mod parsers;
pub mod registry;
pub mod tools;

pub use base::{StandardAgent, StandardAgentConfig, handle_standard_event, run_standard_agent};
pub use parsers::{
    create_create_branch_parser, create_done_parser, create_list_branches_parser,
    create_list_parser, create_read_parser, create_shell_parser, create_write_parser,
};
pub use registry::{ToolRegistryBuilder, ToolRegistryConfig, ToolRegistryFlags};
