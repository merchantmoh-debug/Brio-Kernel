//! Sandbox policy for file system access control.
//!
//! This module enforces security policies on file system paths, ensuring
//! agents can only access authorized directories within configured boundaries.

use crate::infrastructure::config::SandboxSettings;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PolicyError {
    #[error("Invalid path '{path}': {source}")]
    InvalidPath {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("Security Violation: Path '{target:?}' is outside the authorized sandbox roots.")]
    SecurityViolation { target: PathBuf },
    #[error("Invalid allowed path configuration '{path}': {source}")]
    InvalidConfig {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

/// Enforces sandbox security policies on file paths.
#[derive(Debug)]
pub struct SandboxPolicy {
    allowed_paths: Vec<PathBuf>,
}

impl SandboxPolicy {
    /// Creates a new policy from settings.
    ///
    /// # Errors
    ///
    /// Returns an error if any configured allowed path cannot be canonicalized.
    pub fn new(settings: &SandboxSettings) -> Result<Self, PolicyError> {
        let mut allowed_paths = Vec::with_capacity(settings.allowed_paths.len());

        for path_str in &settings.allowed_paths {
            let path = PathBuf::from(path_str);
            let canonical = dunce::canonicalize(&path).map_err(|e| PolicyError::InvalidConfig {
                path: path.clone(),
                source: e,
            })?;
            allowed_paths.push(canonical);
        }

        Ok(Self { allowed_paths })
    }

    /// Creates an empty policy that allows everything.
    pub fn new_empty() -> Self {
        Self {
            allowed_paths: Vec::new(),
        }
    }

    /// Validates that the given path is within one of the allowed sandbox roots.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The path cannot be canonicalized
    /// - The canonicalized path is outside all allowed sandbox roots
    pub fn validate_path(&self, target: &Path) -> Result<(), PolicyError> {
        if self.allowed_paths.is_empty() {
            return Ok(());
        }

        // Canonicalize target to resolve symlinks/.. etc
        let canonical_target =
            dunce::canonicalize(target).map_err(|e| PolicyError::InvalidPath {
                path: target.to_path_buf(),
                source: e,
            })?;

        for allowed_path in &self.allowed_paths {
            if canonical_target.starts_with(allowed_path) {
                return Ok(());
            }
        }

        Err(PolicyError::SecurityViolation {
            target: canonical_target,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::config::SandboxSettings;
    use tempfile::tempdir;

    #[test]
    fn test_sandbox_policy_validation() -> anyhow::Result<()> {
        let dir = tempdir()?;
        let root = dir.path().to_path_buf();
        let allowed = root.join("allowed");
        fs_extra::dir::create_all(&allowed, false).map_err(|e| anyhow::anyhow!(e))?;

        let settings = SandboxSettings {
            allowed_paths: vec![
                allowed
                    .to_str()
                    .ok_or_else(|| anyhow::anyhow!("Invalid path"))?
                    .to_string(),
            ],
        };

        let policy = SandboxPolicy::new(&settings).map_err(|e| anyhow::anyhow!(e))?;

        // 1. Valid path within allowed root
        let inside = allowed.join("file.txt");
        fs_extra::file::write_all(&inside, "").map_err(|e| anyhow::anyhow!(e))?;
        assert!(policy.validate_path(&inside).is_ok());

        // 2. Invalid path outside allowed root
        let outside = root.join("secret.txt");
        fs_extra::file::write_all(&outside, "").map_err(|e| anyhow::anyhow!(e))?;
        assert!(policy.validate_path(&outside).is_err());

        // 3. Normalized path (..)
        let tricky = allowed.join("../secret.txt");
        assert!(policy.validate_path(&tricky).is_err());
        Ok(())
    }

    #[test]
    fn test_empty_policy_allows_everything() -> anyhow::Result<()> {
        let settings = SandboxSettings::default();
        let policy = SandboxPolicy::new(&settings).map_err(|e| anyhow::anyhow!(e))?;
        assert!(policy.validate_path(Path::new("/tmp/some_path")).is_ok());
        Ok(())
    }
}
