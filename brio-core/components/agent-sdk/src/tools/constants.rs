//! Tool name constants for the Agent SDK.
//!
//! Using constants prevents typos and makes refactoring easier.
//! All tool names should be defined here and imported where needed.

/// Tool names for control operations.
pub mod control {
    /// Mark task as complete.
    pub const DONE: &str = "done";
}

/// Tool names for file system operations.
pub mod fs {
    /// Read content from a file.
    pub const READ_FILE: &str = "read_file";

    /// Write content to a file.
    pub const WRITE_FILE: &str = "write_file";

    /// List directory contents.
    pub const LS: &str = "ls";
}

/// Tool names for shell operations.
pub mod shell {
    /// Execute a shell command.
    pub const SHELL: &str = "shell";
}

/// Tool names for grep operations.
pub mod grep {
    /// Search file contents.
    pub const GREP: &str = "grep";
}

/// Tool names for branch operations.
pub mod branch {
    /// Create a new branch.
    pub const CREATE_BRANCH: &str = "create_branch";

    /// List all branches.
    pub const LIST_BRANCHES: &str = "list_branches";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_control_constants() {
        assert_eq!(control::DONE, "done");
    }

    #[test]
    fn test_fs_constants() {
        assert_eq!(fs::READ_FILE, "read_file");
        assert_eq!(fs::WRITE_FILE, "write_file");
        assert_eq!(fs::LS, "ls");
    }

    #[test]
    fn test_shell_constants() {
        assert_eq!(shell::SHELL, "shell");
    }

    #[test]
    fn test_branch_constants() {
        assert_eq!(branch::CREATE_BRANCH, "create_branch");
        assert_eq!(branch::LIST_BRANCHES, "list_branches");
    }

    #[test]
    fn test_grep_constants() {
        assert_eq!(grep::GREP, "grep");
    }
}
