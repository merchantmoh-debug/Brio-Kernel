//! Configuration management for agents.
//!
//! Provides a strongly-typed configuration system with environment variable
//! support and sensible defaults.

use crate::error::TaskError;
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Agent configuration settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    /// Maximum number of iterations before stopping.
    #[serde(default = "default_max_iterations")]
    pub max_iterations: u32,

    /// AI model to use for inference.
    #[serde(default = "default_model")]
    pub model: String,

    /// Timeout for agent execution.
    #[serde(default = "default_timeout", with = "humantime_serde")]
    pub timeout: Duration,

    /// Whether to enable verbose logging.
    #[serde(default = "default_verbose")]
    pub verbose: bool,

    /// Maximum file size to read (in bytes).
    #[serde(default = "default_max_file_size")]
    pub max_file_size: u64,

    /// Maximum depth for directory traversal.
    #[serde(default = "default_max_depth")]
    pub max_depth: usize,

    /// Shell command allowlist (if empty, all safe commands allowed).
    #[serde(default)]
    pub shell_allowlist: Vec<String>,

    /// Tool-specific configurations.
    #[serde(default)]
    pub tool_config: ToolConfig,
}

/// Tool-specific configuration.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ToolConfig {
    /// Enable write operations.
    #[serde(default = "default_true")]
    pub enable_write: bool,

    /// Enable shell command execution.
    #[serde(default = "default_true")]
    pub enable_shell: bool,

    /// Enable directory listing.
    #[serde(default = "default_true")]
    pub enable_list: bool,
}

impl AgentConfig {
    /// Creates a new configuration with default values.
    pub fn new() -> Self {
        Self::default()
    }

    /// Loads configuration from environment variables.
    ///
    /// Environment variables are prefixed with `BRIO_AGENT_`.
    /// For example: `BRIO_AGENT_MAX_ITERATIONS=30`
    pub fn from_env() -> Result<Self, TaskError> {
        let mut config = Self::default();

        if let Ok(val) = std::env::var("BRIO_AGENT_MAX_ITERATIONS") {
            config.max_iterations = val.parse().map_err(|_| TaskError::InvalidConfiguration {
                key: "max_iterations".to_string(),
                value: val,
            })?;
        }

        if let Ok(val) = std::env::var("BRIO_AGENT_MODEL") {
            config.model = val;
        }

        if let Ok(val) = std::env::var("BRIO_AGENT_TIMEOUT_SECONDS") {
            let seconds: u64 = val.parse().map_err(|_| TaskError::InvalidConfiguration {
                key: "timeout_seconds".to_string(),
                value: val.clone(),
            })?;
            config.timeout = Duration::from_secs(seconds);
        }

        if let Ok(val) = std::env::var("BRIO_AGENT_VERBOSE") {
            config.verbose = val == "1" || val.to_lowercase() == "true";
        }

        if let Ok(val) = std::env::var("BRIO_AGENT_MAX_FILE_SIZE") {
            config.max_file_size = val.parse().map_err(|_| TaskError::InvalidConfiguration {
                key: "max_file_size".to_string(),
                value: val,
            })?;
        }

        if let Ok(val) = std::env::var("BRIO_AGENT_MAX_DEPTH") {
            config.max_depth = val.parse().map_err(|_| TaskError::InvalidConfiguration {
                key: "max_depth".to_string(),
                value: val,
            })?;
        }

        Ok(config)
    }

    /// Validates the configuration.
    pub fn validate(&self) -> Result<&Self, TaskError> {
        if self.max_iterations == 0 {
            return Err(TaskError::InvalidConfiguration {
                key: "max_iterations".to_string(),
                value: "0".to_string(),
            });
        }

        if self.model.trim().is_empty() {
            return Err(TaskError::InvalidConfiguration {
                key: "model".to_string(),
                value: "empty".to_string(),
            });
        }

        if self.max_file_size == 0 {
            return Err(TaskError::InvalidConfiguration {
                key: "max_file_size".to_string(),
                value: "0".to_string(),
            });
        }

        Ok(self)
    }

    /// Returns a builder for creating configuration.
    pub fn builder() -> AgentConfigBuilder {
        AgentConfigBuilder::default()
    }
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            max_iterations: default_max_iterations(),
            model: default_model(),
            timeout: default_timeout(),
            verbose: default_verbose(),
            max_file_size: default_max_file_size(),
            max_depth: default_max_depth(),
            shell_allowlist: default_shell_allowlist(),
            tool_config: ToolConfig::default(),
        }
    }
}

/// Builder for constructing AgentConfig.
#[derive(Debug, Default)]
pub struct AgentConfigBuilder {
    max_iterations: Option<u32>,
    model: Option<String>,
    timeout: Option<Duration>,
    verbose: Option<bool>,
    max_file_size: Option<u64>,
    max_depth: Option<usize>,
    shell_allowlist: Option<Vec<String>>,
    tool_config: Option<ToolConfig>,
}

impl AgentConfigBuilder {
    /// Sets the maximum number of iterations.
    pub fn max_iterations(mut self, iterations: u32) -> Self {
        self.max_iterations = Some(iterations);
        self
    }

    /// Sets the AI model to use.
    pub fn model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }

    /// Sets the execution timeout.
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// Sets whether to enable verbose logging.
    pub fn verbose(mut self, verbose: bool) -> Self {
        self.verbose = Some(verbose);
        self
    }

    /// Sets the maximum file size for reading.
    pub fn max_file_size(mut self, size: u64) -> Self {
        self.max_file_size = Some(size);
        self
    }

    /// Sets the maximum directory traversal depth.
    pub fn max_depth(mut self, depth: usize) -> Self {
        self.max_depth = Some(depth);
        self
    }

    /// Sets the shell command allowlist.
    pub fn shell_allowlist(mut self, allowlist: Vec<String>) -> Self {
        self.shell_allowlist = Some(allowlist);
        self
    }

    /// Sets the tool-specific configuration.
    pub fn tool_config(mut self, config: ToolConfig) -> Self {
        self.tool_config = Some(config);
        self
    }

    /// Builds the configuration, validating all values.
    pub fn build(self) -> Result<AgentConfig, TaskError> {
        let mut config = AgentConfig::default();

        if let Some(v) = self.max_iterations {
            config.max_iterations = v;
        }
        if let Some(v) = self.model {
            config.model = v;
        }
        if let Some(v) = self.timeout {
            config.timeout = v;
        }
        if let Some(v) = self.verbose {
            config.verbose = v;
        }
        if let Some(v) = self.max_file_size {
            config.max_file_size = v;
        }
        if let Some(v) = self.max_depth {
            config.max_depth = v;
        }
        if let Some(v) = self.shell_allowlist {
            config.shell_allowlist = v;
        }
        if let Some(v) = self.tool_config {
            config.tool_config = v;
        }

        // Validate first
        config.validate()?;
        Ok(config)
    }
}

// Default value functions
fn default_max_iterations() -> u32 {
    20
}

fn default_model() -> String {
    "best-available".to_string()
}

fn default_timeout() -> Duration {
    Duration::from_secs(300) // 5 minutes
}

fn default_verbose() -> bool {
    false
}

fn default_max_file_size() -> u64 {
    10 * 1024 * 1024 // 10MB
}

fn default_max_depth() -> usize {
    10
}

fn default_shell_allowlist() -> Vec<String> {
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
}

fn default_true() -> bool {
    true
}

// Human-friendly duration serialization
mod humantime_serde {
    use serde::{Deserialize, Deserializer, Serializer};
    use std::time::Duration;

    pub fn serialize<S>(duration: &Duration, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u64(duration.as_secs())
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Duration, D::Error>
    where
        D: Deserializer<'de>,
    {
        let secs = u64::deserialize(deserializer)?;
        Ok(Duration::from_secs(secs))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = AgentConfig::default();
        assert_eq!(config.max_iterations, 20);
        assert_eq!(config.model, "best-available");
        assert_eq!(config.max_file_size, 10 * 1024 * 1024);
    }

    #[test]
    fn test_builder_pattern() {
        let config = AgentConfig::builder()
            .max_iterations(30)
            .model("gpt-4")
            .verbose(true)
            .build()
            .unwrap();

        assert_eq!(config.max_iterations, 30);
        assert_eq!(config.model, "gpt-4");
        assert!(config.verbose);
    }

    #[test]
    fn test_invalid_config() {
        let result = AgentConfig::builder().max_iterations(0).build();
        assert!(result.is_err());
    }
}
