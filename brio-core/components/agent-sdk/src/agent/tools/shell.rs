//! Shell execution tools for agent operations.
//!
//! This module provides secure shell command execution with allowlist
//! validation to prevent command injection attacks.

use crate::error::ToolError;
use crate::tools::constants::shell;
use crate::tools::{Tool, validate_shell_command};
use std::borrow::Cow;
use std::collections::HashMap;

/// Tool for executing shell commands.
///
/// Executes shell commands after validating them against an allowlist.
/// Commands are checked for both allowlist membership and dangerous
/// characters that could indicate injection attacks.
///
/// # Security
///
/// - Commands must be in the configured allowlist
/// - Dangerous characters (`;`, `&`, `|`, `>`, `<`, `` ` ``, `$`, `(`) are blocked
/// - Command output is captured and returned
/// - Non-zero exit codes result in errors
///
/// # Example
///
/// ```
/// use agent_sdk::agent::tools::ShellTool;
/// use agent_sdk::Tool;
/// use std::collections::HashMap;
///
/// # fn example() {
/// let allowlist = vec!["ls".to_string(), "cat".to_string(), "echo".to_string()];
/// let tool = ShellTool::new(allowlist);
/// # }
/// ```
pub struct ShellTool {
    allowlist: Vec<String>,
}

impl ShellTool {
    /// Creates a new `ShellTool` with the specified command allowlist.
    ///
    /// # Arguments
    ///
    /// * `allowlist` - List of allowed command prefixes (e.g., `["ls", "cat", "echo"]`)
    ///
    /// # Example
    ///
    /// ```
    /// use agent_sdk::agent::tools::ShellTool;
    ///
    /// let allowlist = vec!["ls".to_string(), "cat".to_string()];
    /// let tool = ShellTool::new(allowlist);
    /// ```
    #[must_use]
    pub fn new(allowlist: Vec<String>) -> Self {
        Self { allowlist }
    }

    /// Returns a reference to the configured allowlist.
    #[must_use]
    pub fn allowlist(&self) -> &[String] {
        &self.allowlist
    }
}

impl Tool for ShellTool {
    fn name(&self) -> Cow<'static, str> {
        Cow::Borrowed(shell::SHELL)
    }

    fn description(&self) -> Cow<'static, str> {
        Cow::Borrowed("<shell>command</shell> - Execute a shell command")
    }

    fn execute(&self, args: &HashMap<String, String>) -> Result<String, ToolError> {
        let command = args
            .get("command")
            .ok_or_else(|| ToolError::InvalidArguments {
                tool: shell::SHELL.to_string(),
                reason: "Missing 'command' argument".to_string(),
            })?;

        // Validate command against allowlist
        validate_shell_command(command, &self.allowlist).map_err(|e| ToolError::Blocked {
            tool: shell::SHELL.to_string(),
            reason: e.to_string(),
        })?;

        // Execute the command
        let output = std::process::Command::new("sh")
            .arg("-c")
            .arg(command)
            .output()
            .map_err(|e| ToolError::ExecutionFailed {
                tool: shell::SHELL.to_string(),
                source: Box::new(e),
            })?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        // Check exit status
        if output.status.success() {
            Ok(stdout.to_string())
        } else {
            Err(ToolError::ExecutionFailed {
                tool: shell::SHELL.to_string(),
                source: Box::new(std::io::Error::other(format!(
                    "Exit code {:?}: {}",
                    output.status.code(),
                    stderr
                ))),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shell_tool_name() {
        let tool = ShellTool::new(vec!["ls".to_string()]);
        assert_eq!(tool.name(), shell::SHELL);
    }

    #[test]
    fn test_shell_tool_description() {
        let tool = ShellTool::new(vec![]);
        assert!(tool.description().contains(shell::SHELL));
        assert!(tool.description().contains("command"));
    }

    #[test]
    fn test_shell_tool_allowlist() {
        let allowlist = vec!["ls".to_string(), "cat".to_string()];
        let tool = ShellTool::new(allowlist.clone());
        assert_eq!(tool.allowlist(), allowlist.as_slice());
    }

    #[test]
    fn test_shell_missing_command() {
        let tool = ShellTool::new(vec!["ls".to_string()]);
        let args = HashMap::new();
        let result = tool.execute(&args);

        assert!(matches!(
            result,
            Err(ToolError::InvalidArguments { tool, reason })
            if tool == shell::SHELL && reason.contains("command")
        ));
    }

    #[test]
    fn test_shell_blocked_command() {
        let tool = ShellTool::new(vec!["ls".to_string(), "cat".to_string()]);
        let mut args = HashMap::new();
        args.insert("command".to_string(), "rm -rf /".to_string());

        let result = tool.execute(&args);

        assert!(matches!(
            result,
            Err(ToolError::Blocked { tool, reason })
            if tool == shell::SHELL && reason.contains("allowed")
        ));
    }

    #[test]
    fn test_shell_injection_attack() {
        let tool = ShellTool::new(vec!["ls".to_string()]);
        let mut args = HashMap::new();
        args.insert("command".to_string(), "ls; rm -rf /".to_string());

        let result = tool.execute(&args);

        // The command "ls;" is not in the allowlist, so it gets blocked
        assert!(matches!(
            result,
            Err(ToolError::Blocked { tool, reason })
            if tool == shell::SHELL && reason.contains("allowed")
        ));
    }

    #[test]
    fn test_shell_pipe_attack() {
        let tool = ShellTool::new(vec!["cat".to_string()]);
        let mut args = HashMap::new();
        args.insert(
            "command".to_string(),
            "cat /etc/passwd | grep root".to_string(),
        );

        let result = tool.execute(&args);

        assert!(matches!(
            result,
            Err(ToolError::Blocked { tool, reason })
            if tool == shell::SHELL && reason.contains("dangerous")
        ));
    }

    #[test]
    fn test_shell_backtick_attack() {
        let tool = ShellTool::new(vec!["echo".to_string()]);
        let mut args = HashMap::new();
        args.insert("command".to_string(), "echo `whoami`".to_string());

        let result = tool.execute(&args);

        assert!(matches!(
            result,
            Err(ToolError::Blocked { tool, reason })
            if tool == shell::SHELL && reason.contains("dangerous")
        ));
    }

    #[test]
    fn test_shell_dollar_attack() {
        let tool = ShellTool::new(vec!["echo".to_string()]);
        let mut args = HashMap::new();
        args.insert("command".to_string(), "echo $(whoami)".to_string());

        let result = tool.execute(&args);

        assert!(matches!(
            result,
            Err(ToolError::Blocked { tool, reason })
            if tool == shell::SHELL && reason.contains("dangerous")
        ));
    }

    #[test]
    fn test_shell_redirect_attack() {
        let tool = ShellTool::new(vec!["cat".to_string()]);
        let mut args = HashMap::new();
        args.insert("command".to_string(), "cat > /etc/passwd".to_string());

        let result = tool.execute(&args);

        assert!(matches!(
            result,
            Err(ToolError::Blocked { tool, reason })
            if tool == shell::SHELL && reason.contains("dangerous")
        ));
    }

    #[test]
    fn test_shell_valid_command() {
        let tool = ShellTool::new(vec!["echo".to_string()]);
        let mut args = HashMap::new();
        args.insert("command".to_string(), "echo 'Hello, World!'".to_string());

        let result = tool.execute(&args);
        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(output.contains("Hello, World!"));
    }

    #[test]
    fn test_shell_ls_command() {
        let tool = ShellTool::new(vec!["ls".to_string()]);
        let mut args = HashMap::new();
        args.insert("command".to_string(), "ls -la".to_string());

        let result = tool.execute(&args);
        assert!(result.is_ok());
    }

    #[test]
    fn test_shell_failed_command() {
        let tool = ShellTool::new(vec!["ls".to_string()]);
        let mut args = HashMap::new();
        args.insert(
            "command".to_string(),
            "ls /nonexistent_directory_12345".to_string(),
        );

        let result = tool.execute(&args);
        assert!(result.is_err());
    }
}
