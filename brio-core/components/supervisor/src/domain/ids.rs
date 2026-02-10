//! Domain identifiers - Newtype wrappers for type safety
//!
//! This module provides strongly-typed identifiers to prevent mixing up
//! different types of IDs at compile time.

use core::fmt;
use serde::{Deserialize, Serialize};

/// Unique identifier for a branch in the system.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BranchId(uuid::Uuid);

impl BranchId {
    /// Creates a new `BranchId` with a random UUID.
    #[must_use]
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4())
    }

    /// Creates a `BranchId` from an existing UUID.
    #[must_use]
    pub const fn from_uuid(uuid: uuid::Uuid) -> Self {
        Self(uuid)
    }

    /// Returns the inner UUID.
    #[must_use]
    pub const fn inner(self) -> uuid::Uuid {
        self.0
    }
}

impl Default for BranchId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for BranchId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Unique identifier for a task in the system.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TaskId(
    /// The underlying numeric identifier (auto-incrementing).
    u64,
);

impl TaskId {
    /// Creates a new `TaskId` from a raw value.
    #[must_use]
    pub const fn new(id: u64) -> Self {
        Self(id)
    }

    /// Returns the inner value.
    #[must_use]
    pub const fn inner(self) -> u64 {
        self.0
    }
}

impl fmt::Display for TaskId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "task_{}", self.0)
    }
}

/// Unique identifier for an agent in the mesh.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AgentId(String);

impl AgentId {
    /// Creates a new `AgentId` from a string.
    ///
    /// # Errors
    /// Returns `ValidationError::EmptyAgentId` if the id is empty.
    pub fn new(id: impl Into<String>) -> Result<Self, super::ValidationError> {
        let id = id.into();
        if id.is_empty() {
            return Err(super::ValidationError::EmptyAgentId);
        }
        Ok(Self(id))
    }

    /// Returns the inner string reference.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for AgentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Task priority (0-255, higher = more urgent).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Priority(u8);

impl Priority {
    /// Lowest priority value.
    pub const MIN: Self = Self(0);
    /// Highest priority value.
    pub const MAX: Self = Self(255);
    /// Default priority for new tasks.
    pub const DEFAULT: Self = Self(128);

    /// Creates a new Priority from a raw value.
    #[must_use]
    pub const fn new(value: u8) -> Self {
        Self(value)
    }

    /// Returns the inner value.
    #[must_use]
    pub const fn inner(self) -> u8 {
        self.0
    }
}

impl Default for Priority {
    fn default() -> Self {
        Self::DEFAULT
    }
}
