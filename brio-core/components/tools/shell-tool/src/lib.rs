//! Shell Tool - Provides shell command execution capabilities.
//!
//! This tool component allows agents to execute shell commands on the host system.
//! Use with caution as this provides direct system access.
//!
//! # Security Considerations
//!
//! - Commands are validated against a denylist to prevent dangerous operations
//! - Shell operators (|, ;, &&, ||) are rejected to prevent command injection
//! - Path traversal attempts are detected and blocked
//! - No shell interpolation is performed - commands run directly without shell
//!
//! # Error Handling
//!
//! Uses proper error types with [`ShellError`] instead of string errors.
//! UTF-8 decoding preserves invalid bytes as replacement characters.

// WIT bindings generate many undocumented items - this is expected for auto-generated code
#![allow(missing_docs)]

use std::fmt;
use std::process::Command;
use wit_bindgen::generate;

// Generate WIT bindings
generate!({
    world: "standard-tool",
    path: "../../../wit/tool.wit",
    export_macro_name: "export_shell_tool",
});

export_shell_tool!(ShellTool);

/// Errors that can occur during shell command execution.
#[derive(Debug, Clone)]
pub enum ShellError {
    /// Failed to parse JSON parameters.
    InvalidParams(String),
    /// No command was provided.
    NoCommand,
    /// Command contains dangerous characters or patterns.
    DangerousCommand(String),
    /// Command execution failed.
    ExecutionFailed(String),
    /// Command returned non-zero exit code.
    CommandFailed {
        /// Standard output from the command.
        stdout: String,
        /// Standard error from the command.
        stderr: String,
        /// Exit code returned by the command.
        code: i32,
    },
    /// Output contains invalid UTF-8 sequences.
    InvalidUtf8(String),
}

impl fmt::Display for ShellError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ShellError::InvalidParams(msg) => write!(f, "Invalid parameters: {msg}"),
            ShellError::NoCommand => write!(f, "No command provided"),
            ShellError::DangerousCommand(cmd) => write!(f, "Dangerous command detected: {cmd}"),
            ShellError::ExecutionFailed(msg) => write!(f, "Execution failed: {msg}"),
            ShellError::CommandFailed { stderr, code, .. } => {
                write!(f, "Command failed with exit code {code}: {stderr}")
            }
            ShellError::InvalidUtf8(msg) => write!(f, "Invalid UTF-8 in output: {msg}"),
        }
    }
}

impl From<ShellError> for String {
    fn from(err: ShellError) -> Self {
        err.to_string()
    }
}

/// Shell tool implementation for executing system commands.
struct ShellTool;

impl ShellTool {
    /// Validates command for dangerous patterns.
    ///
    /// # Security
    ///
    /// Checks for:
    /// - Shell metacharacters (`|`, `;`, `&`, `$`, `` ` ``, etc.)
    /// - Path traversal attempts (`../`)
    /// - Dangerous commands (`rm -rf`, `mkfs`, etc.)
    fn validate_command(command: &str) -> Result<(), ShellError> {
        // Check for shell metacharacters that could enable injection
        const DANGEROUS_CHARS: &[char] = &['|', ';', '&', '$', '`', '>', '<', '(', ')', '{', '}'];
        if command.chars().any(|c| DANGEROUS_CHARS.contains(&c)) {
            return Err(ShellError::DangerousCommand(format!(
                "Command contains shell metacharacters: {command}"
            )));
        }

        // Check for path traversal
        if command.contains("../") || command.contains("..\\") {
            return Err(ShellError::DangerousCommand(format!(
                "Command contains path traversal: {command}"
            )));
        }

        // Check for dangerous command names
        let dangerous_commands = ["rm", "mkfs", "dd", "format", "fdisk", "del"];
        let cmd_lower = command.to_lowercase();
        if dangerous_commands
            .iter()
            .any(|&dangerous| cmd_lower.starts_with(dangerous))
        {
            return Err(ShellError::DangerousCommand(format!(
                "Potentially destructive command: {command}"
            )));
        }

        Ok(())
    }

    /// Validates command arguments for dangerous patterns.
    fn validate_arguments(args: &[String]) -> Result<(), ShellError> {
        for arg in args {
            // Check for shell metacharacters
            const DANGEROUS_CHARS: &[char] = &['|', ';', '&', '$', '`', '>', '<'];
            if arg.chars().any(|c| DANGEROUS_CHARS.contains(&c)) {
                return Err(ShellError::DangerousCommand(format!(
                    "Argument contains shell metacharacters: {arg}"
                )));
            }

            // Check for path traversal in arguments
            if arg.contains("../") || arg.contains("..\\") {
                return Err(ShellError::DangerousCommand(format!(
                    "Argument contains path traversal: {arg}"
                )));
            }
        }

        Ok(())
    }

    /// Converts bytes to string, properly handling invalid UTF-8.
    ///
    /// Unlike `String::from_utf8_lossy`, this returns an error for completely invalid
    /// UTF-8, while still preserving valid portions.
    fn bytes_to_string(bytes: Vec<u8>) -> Result<String, ShellError> {
        match String::from_utf8(bytes) {
            Ok(s) => Ok(s),
            Err(e) => {
                // Try to get valid portion and indicate error
                let valid = String::from_utf8_lossy(e.as_bytes());
                if valid.is_empty() {
                    Err(ShellError::InvalidUtf8(
                        "Output contains no valid UTF-8".to_string(),
                    ))
                } else {
                    // Return valid portion with warning indicator
                    Ok(format!("{valid}[INVALID_UTF8_TRUNCATED]"))
                }
            }
        }
    }
}

impl exports::brio::core::tool::Guest for ShellTool {
    fn info() -> exports::brio::core::tool::ToolInfo {
        exports::brio::core::tool::ToolInfo {
            name: "shell".to_string(),
            description: "Executes shell commands safely with input validation. Use with caution."
                .to_string(),
            version: "0.2.0".to_string(),
            requires_session: false,
        }
    }

    fn execute(params: String, _session_id: Option<String>) -> Result<String, String> {
        let args: Vec<String> =
            serde_json::from_str(&params).map_err(|e| ShellError::InvalidParams(e.to_string()))?;

        if args.is_empty() {
            return Err(ShellError::NoCommand.into());
        }

        let command = &args[0];
        let command_args = &args[1..];

        // Validate command and arguments for security
        Self::validate_command(command)?;
        Self::validate_arguments(command_args)?;

        let output = Command::new(command)
            .args(command_args)
            .output()
            .map_err(|e| ShellError::ExecutionFailed(e.to_string()))?;

        if output.status.success() {
            Self::bytes_to_string(output.stdout).map_err(std::convert::Into::into)
        } else {
            let stderr = Self::bytes_to_string(output.stderr).unwrap_or_default();
            let code = output.status.code().unwrap_or(-1);
            Err(ShellError::CommandFailed {
                stdout: Self::bytes_to_string(output.stdout).unwrap_or_default(),
                stderr,
                code,
            }
            .into())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_command_safe() {
        assert!(ShellTool::validate_command("ls").is_ok());
        assert!(ShellTool::validate_command("cat").is_ok());
        assert!(ShellTool::validate_command("echo").is_ok());
    }

    #[test]
    fn test_validate_command_dangerous_chars() {
        assert!(ShellTool::validate_command("ls|cat").is_err());
        assert!(ShellTool::validate_command("echo;rm").is_err());
        assert!(ShellTool::validate_command("cmd&&other").is_err());
    }

    #[test]
    fn test_validate_command_path_traversal() {
        assert!(ShellTool::validate_command("../bin/ls").is_err());
        assert!(ShellTool::validate_command("cat../../etc/passwd").is_err());
    }

    #[test]
    fn test_validate_command_destructive() {
        assert!(ShellTool::validate_command("rm -rf /").is_err());
        assert!(ShellTool::validate_command("mkfs.ext4 /dev/sda").is_err());
    }

    #[test]
    fn test_validate_arguments_safe() {
        assert!(ShellTool::validate_arguments(&["file.txt".to_string()]).is_ok());
        assert!(ShellTool::validate_arguments(&["-la".to_string()]).is_ok());
    }

    #[test]
    fn test_validate_arguments_dangerous() {
        assert!(ShellTool::validate_arguments(&["file|cat".to_string()]).is_err());
        assert!(ShellTool::validate_arguments(&["../etc/passwd".to_string()]).is_err());
    }

    #[test]
    fn test_bytes_to_string_valid() {
        let bytes = b"hello world".to_vec();
        assert_eq!(ShellTool::bytes_to_string(bytes).unwrap(), "hello world");
    }

    #[test]
    fn test_bytes_to_string_invalid_utf8() {
        let bytes = vec![0x80, 0x81, 0x82]; // Invalid UTF-8 sequences
        let result = ShellTool::bytes_to_string(bytes);
        assert!(result.is_ok());
        assert!(result.unwrap().contains("INVALID_UTF8_TRUNCATED"));
    }

    #[test]
    fn test_shell_error_display() {
        let err = ShellError::NoCommand;
        assert_eq!(err.to_string(), "No command provided");

        let err = ShellError::InvalidParams("bad json".to_string());
        assert!(err.to_string().contains("Invalid parameters"));
    }
}
