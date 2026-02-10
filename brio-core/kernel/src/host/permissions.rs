//! Permission checking for the Brio kernel.
//!
//! This module provides permission validation for host operations.

use thiserror::Error;

/// Errors related to permission checks.
#[derive(Debug, Error)]
pub enum PermissionError {
    /// The required permission was not granted.
    #[error("Permission denied: required '{permission}'")]
    PermissionDenied {
        /// The permission that was denied.
        permission: String,
    },
}

/// Trait for permission checking functionality.
pub trait PermissionChecker {
    /// Checks if a permission is granted.
    ///
    /// # Errors
    /// Returns error if permission is denied.
    fn check_permission(&self, permission: &str) -> Result<(), String>;
}

/// A simple permission checker that always allows all permissions.
#[derive(Debug, Clone)]
pub struct AllowAllPermissions;

impl PermissionChecker for AllowAllPermissions {
    fn check_permission(&self, _permission: &str) -> Result<(), String> {
        Ok(())
    }
}

/// A permission checker that validates against a set of allowed permissions.
#[derive(Debug, Clone)]
pub struct RestrictedPermissions {
    allowed: std::collections::HashSet<String>,
}

impl RestrictedPermissions {
    /// Create a new restricted permission checker with the given allowed permissions.
    #[must_use]
    pub fn new(allowed: Vec<String>) -> Self {
        Self {
            allowed: allowed.into_iter().collect(),
        }
    }

    /// Check if a specific permission is allowed.
    #[must_use]
    pub fn has_permission(&self, permission: &str) -> bool {
        self.allowed.contains(permission)
    }
}

impl PermissionChecker for RestrictedPermissions {
    fn check_permission(&self, permission: &str) -> Result<(), String> {
        if self.allowed.contains(permission) {
            Ok(())
        } else {
            Err(PermissionError::PermissionDenied {
                permission: permission.to_string(),
            }
            .to_string())
        }
    }
}
