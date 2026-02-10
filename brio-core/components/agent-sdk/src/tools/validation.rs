//! Security validation for tool operations.

use crate::error::{FileSystemError, ToolError};
use crate::tools::constants;
use std::path::{Component, Path, PathBuf};

/// Validates a file path to prevent path traversal attacks.
///
/// # Errors
///
/// Returns an error if the path contains parent directory references (`..`) or
/// if the resolved path is outside the base directory.
pub fn validate_path(path: &str, base_dir: &Path) -> Result<PathBuf, FileSystemError> {
    let path = Path::new(path);

    // Check for path traversal components
    for component in path.components() {
        if let Component::ParentDir = component {
            return Err(FileSystemError::PathTraversal {
                path: path.to_path_buf(),
            });
        }
    }

    // Resolve the path relative to base directory
    let resolved = base_dir.join(path);
    let canonical_base = base_dir.canonicalize().map_err(|e| {
        FileSystemError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("Base directory not found: {e}"),
        ))
    })?;

    // Ensure the resolved path is within the base directory
    if let Ok(canonical_path) = resolved.canonicalize()
        && !canonical_path.starts_with(&canonical_base)
    {
        return Err(FileSystemError::PathTraversal {
            path: path.to_path_buf(),
        });
    }

    Ok(resolved)
}

/// Checks if a file size is within limits.
///
/// # Errors
///
/// Returns an error if the file metadata cannot be read or if the file size
/// exceeds the specified maximum.
pub fn validate_file_size(path: &Path, max_size: u64) -> Result<(), FileSystemError> {
    let metadata = std::fs::metadata(path).map_err(FileSystemError::Io)?;
    let size = metadata.len();

    if size > max_size {
        return Err(FileSystemError::FileTooLarge {
            path: path.to_path_buf(),
            size,
            max_size,
        });
    }

    Ok(())
}

/// Validates a shell command against an allowlist.
///
/// # Errors
///
/// Returns an error if the command is not in the allowlist or if it contains
/// potentially dangerous characters (e.g., `;`, `&`, `|`, `>`, `<`, `` ` ``, `$`, `(`).
pub fn validate_shell_command(
    command: &str,
    allowlist: &[impl AsRef<str>],
) -> Result<(), ToolError> {
    let cmd_trimmed = command.trim();
    let first_word = cmd_trimmed.split_whitespace().next().unwrap_or(cmd_trimmed);

    let is_allowed = allowlist.iter().any(|prefix| first_word == prefix.as_ref());

    if !is_allowed {
        return Err(ToolError::Blocked {
            tool: constants::shell::SHELL.to_string(),
            reason: format!("Command '{first_word}' is not in the allowed list"),
        });
    }

    // Additional security: check for dangerous characters
    let dangerous_chars = [b';', b'&', b'|', b'>', b'<', b'`', b'$', b'('];
    if command.bytes().any(|c| dangerous_chars.contains(&c)) {
        return Err(ToolError::Blocked {
            tool: constants::shell::SHELL.to_string(),
            reason: "Command contains potentially dangerous characters".to_string(),
        });
    }

    Ok(())
}

/// Type-State pattern for secure file operations.
///
/// This ensures that file paths are validated before any operations are performed.
pub struct SecureFilePath<State> {
    path: PathBuf,
    _state: std::marker::PhantomData<State>,
}

/// Unvalidated state.
pub struct Unvalidated;

/// Validated state.
pub struct Validated;

impl SecureFilePath<Unvalidated> {
    /// Creates a new unvalidated file path.
    #[must_use]
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            _state: std::marker::PhantomData,
        }
    }

    /// Validates the path against a base directory.
    ///
    /// # Errors
    ///
    /// Returns an error if the path contains parent directory references or
    /// if the resolved path is outside the base directory.
    pub fn validate(self, base_dir: &Path) -> Result<SecureFilePath<Validated>, FileSystemError> {
        let path_str = self.path.to_str().ok_or_else(|| {
            FileSystemError::InvalidPath(format!(
                "Path contains invalid UTF-8: {}",
                self.path.display()
            ))
        })?;
        let validated_path = validate_path(path_str, base_dir)?;

        Ok(SecureFilePath {
            path: validated_path,
            _state: std::marker::PhantomData,
        })
    }
}

impl SecureFilePath<Validated> {
    /// Returns the validated path.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Reads the file content with size validation.
    ///
    /// # Errors
    ///
    /// Returns an error if the file size exceeds the limit or if the file
    /// cannot be read.
    pub fn read_with_limit(&self, max_size: u64) -> Result<String, FileSystemError> {
        validate_file_size(&self.path, max_size)?;
        std::fs::read_to_string(&self.path).map_err(FileSystemError::Io)
    }

    /// Writes content to the file.
    ///
    /// # Errors
    ///
    /// Returns an error if parent directories cannot be created or if the file
    /// cannot be written.
    pub fn write(&self, content: &str) -> Result<(), FileSystemError> {
        // Create parent directories if needed
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent).map_err(FileSystemError::Io)?;
        }
        std::fs::write(&self.path, content).map_err(FileSystemError::Io)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_path_traversal() {
        let base = Path::new("/tmp/test");
        let result = validate_path("../../../etc/passwd", base);
        assert!(matches!(result, Err(FileSystemError::PathTraversal { .. })));
    }

    #[test]
    fn test_validate_shell_command() {
        let allowlist = vec!["ls", "cat", "echo"];

        assert!(validate_shell_command("ls -la", &allowlist).is_ok());
        assert!(validate_shell_command("rm -rf /", &allowlist).is_err());
        assert!(validate_shell_command("ls; rm -rf /", &allowlist).is_err());
    }

    #[test]
    fn test_secure_file_path() {
        let temp_dir = tempfile::tempdir().unwrap();
        let base = temp_dir.path();

        // Create a test file
        let test_file = base.join("test.txt");
        std::fs::write(&test_file, "Hello").unwrap();

        // Test validation
        let secure = SecureFilePath::new("test.txt").validate(base).unwrap();
        let content = secure.read_with_limit(1024).unwrap();
        assert_eq!(content, "Hello");
    }
}
