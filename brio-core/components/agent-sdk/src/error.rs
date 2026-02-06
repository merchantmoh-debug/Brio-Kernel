//! Error types for the agent SDK.
//!
//! This module provides a structured error hierarchy using `thiserror`
//! for proper error handling throughout the agent system.

use std::path::PathBuf;
use thiserror::Error;

/// Top-level error type for agent operations.
#[derive(Error, Debug)]
pub enum AgentError {
    /// Error during inference API call.
    #[error("Inference failed: {0}")]
    Inference(#[from] InferenceError),

    /// Error during tool execution.
    #[error("Tool execution failed: {0}")]
    ToolExecution(#[from] ToolError),

    /// Error related to task context or configuration.
    #[error("Task error: {0}")]
    Task(#[from] TaskError),

    /// Error related to file system operations.
    #[error("File system error: {0}")]
    FileSystem(#[from] FileSystemError),

    /// Error when agent exceeds maximum iterations.
    #[error("Maximum iterations ({max}) reached. Task may be incomplete.")]
    MaxIterationsExceeded {
        /// The maximum number of iterations allowed.
        max: u32,
    },

    /// Error when agent times out.
    #[error("Agent execution timed out after {elapsed:?}")]
    Timeout {
        /// How long the agent ran before timing out.
        elapsed: std::time::Duration,
    },

    /// Generic error with context.
    #[error("{context}: {source}")]
    Context {
        /// Context message describing what was happening.
        context: String,
        /// The underlying source error.
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },
}

/// Errors related to AI inference operations.
#[derive(Error, Debug)]
pub enum InferenceError {
    /// The inference API returned an error.
    #[error("API error: {0}")]
    ApiError(String),

    /// Invalid model name or model not available.
    #[error("Invalid model: {model}")]
    InvalidModel {
        /// The name of the invalid model.
        model: String,
    },

    /// Rate limit exceeded.
    #[error("Rate limit exceeded. Retry after: {retry_after:?}")]
    RateLimited {
        /// How long to wait before retrying.
        retry_after: Option<std::time::Duration>,
    },

    /// Network or connection error.
    #[error("Network error: {0}")]
    Network(String),

    /// Response parsing error.
    #[error("Failed to parse response: {0}")]
    ParseError(String),
}

/// Errors related to tool execution.
#[derive(Error, Debug)]
pub enum ToolError {
    /// Tool not found in registry.
    #[error("Tool '{name}' not found")]
    NotFound {
        /// Name of the tool that was not found.
        name: String,
    },

    /// Invalid arguments provided to tool.
    #[error("Invalid arguments for tool '{tool}': {reason}")]
    InvalidArguments {
        /// Name of the tool.
        tool: String,
        /// Reason why arguments are invalid.
        reason: String,
    },

    /// Tool execution failed.
    #[error("Tool '{tool}' execution failed: {source}")]
    ExecutionFailed {
        /// Name of the tool that failed.
        tool: String,
        /// The underlying error.
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },

    /// Tool was blocked for security reasons.
    #[error("Tool '{tool}' blocked: {reason}")]
    Blocked {
        /// Name of the blocked tool.
        tool: String,
        /// Reason why the tool was blocked.
        reason: String,
    },

    /// Tool exceeded resource limits.
    #[error("Tool '{tool}' exceeded {resource} limit")]
    ResourceLimitExceeded {
        /// Name of the tool.
        tool: String,
        /// Type of resource that was exceeded.
        resource: String,
    },
}

/// Errors related to task context and configuration.
#[derive(Error, Debug)]
pub enum TaskError {
    /// Task description is empty or invalid.
    #[error("Invalid task description: {0}")]
    InvalidDescription(String),

    /// Task ID is invalid.
    #[error("Invalid task ID: {0}")]
    InvalidTaskId(String),

    /// Missing required configuration.
    #[error("Missing configuration: {key}")]
    MissingConfiguration {
        /// The configuration key that is missing.
        key: String,
    },

    /// Invalid configuration value.
    #[error("Invalid configuration for '{key}': {value}")]
    InvalidConfiguration {
        /// The configuration key.
        key: String,
        /// The invalid value.
        value: String,
    },
}

/// Errors related to file system operations.
#[derive(Error, Debug)]
pub enum FileSystemError {
    /// Path traversal attempt detected.
    #[error("Path traversal detected: {path}")]
    PathTraversal {
        /// The path that attempted traversal.
        path: PathBuf,
    },

    /// File not found.
    #[error("File not found: {path}")]
    NotFound {
        /// The path that was not found.
        path: PathBuf,
    },

    /// File is too large to process.
    #[error("File too large: {path} ({size} bytes, max: {max_size})")]
    FileTooLarge {
        /// The path of the file.
        path: PathBuf,
        /// Actual size of the file.
        size: u64,
        /// Maximum allowed size.
        max_size: u64,
    },

    /// Permission denied.
    #[error("Permission denied: {path}")]
    PermissionDenied {
        /// The path that was denied.
        path: PathBuf,
    },

    /// Generic IO error.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Invalid path format.
    #[error("Invalid path: {0}")]
    InvalidPath(String),
}

/// Convenience extension trait for adding context to errors.
pub trait ResultExt<T, E> {
    /// Add context to an error.
    fn with_context<C, F>(self, f: F) -> Result<T, AgentError>
    where
        F: FnOnce() -> C,
        C: std::fmt::Display;
}

impl<T, E> ResultExt<T, E> for Result<T, E>
where
    E: std::error::Error + Send + Sync + 'static,
{
    fn with_context<C, F>(self, f: F) -> Result<T, AgentError>
    where
        F: FnOnce() -> C,
        C: std::fmt::Display,
    {
        self.map_err(|e| AgentError::Context {
            context: f().to_string(),
            source: Box::new(e),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_inference_error_display() {
        let err = InferenceError::InvalidModel {
            model: "gpt-99".to_string(),
        };
        assert_eq!(err.to_string(), "Invalid model: gpt-99");
    }

    #[test]
    fn test_tool_error_display() {
        let err = ToolError::NotFound {
            name: "unknown_tool".to_string(),
        };
        assert_eq!(err.to_string(), "Tool 'unknown_tool' not found");
    }

    #[test]
    fn test_max_iterations_error() {
        let err = AgentError::MaxIterationsExceeded { max: 20 };
        assert_eq!(
            err.to_string(),
            "Maximum iterations (20) reached. Task may be incomplete."
        );
    }

    #[test]
    fn test_path_traversal_error() {
        let err = FileSystemError::PathTraversal {
            path: PathBuf::from("../../../etc/passwd"),
        };
        assert!(err.to_string().contains("Path traversal detected"));
    }
}
