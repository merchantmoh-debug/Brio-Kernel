//! Agent SDK - Shared library for building Brio-Kernel agents.
//!
//! This crate provides common utilities and abstractions for creating
//! autonomous agents that can interact with the Brio kernel system.
//!
//! # Features
//!
//! - **Error Handling**: Structured error hierarchy using `thiserror`
//! - **Configuration**: Environment-based configuration with validation
//! - **Tool System**: Type-safe tool execution with security validation
//! - **Agent Engine**: `ReAct` loop implementation with state management
//! - **Prompt Building**: Dynamic prompt construction for different agent types
//!
//! # Example
//!
//! ```rust
//! use agent_sdk::{
//!     AgentEngine, AgentConfig, ToolRegistry, TaskContext,
//!     AgentEngineBuilder, PromptBuilder,
//! };
//!
//! # fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Create a task context
//! let context = TaskContext::new("task-123", "Write a function")
//!     .with_files(vec!["input.rs"]);
//!
//! // Configure the agent
//! let config = AgentConfig::builder()
//!     .max_iterations(30)
//!     .verbose(true)
//!     .build()?;
//!
//! // Build and run the agent
//! let engine = AgentEngineBuilder::new()
//!     .tools(ToolRegistry::new())
//!     .config(config)
//!     .build(&context)?;
//!
//! # Ok(())
//! # }
//! ```

#![deny(missing_docs)]
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![warn(clippy::pedantic)]

pub mod agent;
pub mod config;
pub mod engine;
pub mod error;
pub mod prompt;
pub mod tools;
pub mod types;

// Re-export commonly used types
pub use config::{AgentConfig, AgentConfigBuilder, ToolConfig};
pub use engine::{AgentEngine, AgentEngineBuilder, InferenceFn};
pub use error::{AgentError, FileSystemError, InferenceError, ResultExt, TaskError, ToolError};
pub use prompt::PromptBuilder;
pub use tools::{
    SecureFilePath, Tool, ToolParser, ToolRegistry, Unvalidated, Validated, validate_file_size,
    validate_path, validate_shell_command,
};
pub use types::{
    ExecutionResult, InferenceResponse, Message, Role, TaskContext, ToolInvocation, ToolResult,
};

/// Version of the agent SDK.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Initializes logging for the SDK.
///
/// This should be called once at the start of the application.
///
/// # Errors
///
/// Returns an error if the tracing subscriber has already been set.
pub fn init_logging() -> Result<(), tracing::subscriber::SetGlobalDefaultError> {
    tracing::subscriber::set_global_default(tracing_subscriber::FmtSubscriber::builder().finish())
}

/// Utility function to get the current working directory.
///
/// # Errors
///
/// Returns an error if the current working directory cannot be determined
/// (e.g., if the directory has been deleted).
pub fn current_dir() -> Result<std::path::PathBuf, std::io::Error> {
    std::env::current_dir()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version() {
        assert!(!VERSION.is_empty());
        assert!(VERSION.contains('.'));
    }

    #[test]
    fn test_current_dir() {
        let dir = current_dir().expect("current dir should be accessible");
        assert!(dir.exists());
    }
}
