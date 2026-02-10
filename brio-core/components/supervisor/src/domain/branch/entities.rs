//! Branch domain - Branch entities
//!
//! This module defines the core branch-related entities including
//! `BranchRecord`, `AgentAssignment`, `BranchConfig`, and `ExecutionStrategy`.

use super::validation::{MAX_BRANCH_NAME_LEN, MAX_CONCURRENT_BRANCHES, MIN_BRANCH_NAME_LEN};
use crate::domain::ids::{AgentId, BranchId, Priority};
use crate::domain::{BranchValidationError, ValidationError};
use serde::{Deserialize, Serialize};

/// Database record representing a branch row.
///
/// This is the persistence-layer representation of a branch.
/// For the rich domain entity with business logic, see `branch::Branch`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchRecord {
    id: BranchId,
    parent_id: Option<BranchId>,
    session_id: String,
    name: String,
    status: super::status::BranchStatus,
    created_at: i64,
    completed_at: Option<i64>,
    config: String,
}

impl BranchRecord {
    /// Constructs a new `BranchRecord` (factory method).
    ///
    /// # Errors
    /// Returns `ValidationError::EmptyBranchName` if name is empty.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: BranchId,
        parent_id: Option<BranchId>,
        session_id: String,
        name: String,
        status: super::status::BranchStatus,
        created_at: i64,
        completed_at: Option<i64>,
        config: String,
    ) -> Result<Self, ValidationError> {
        if name.is_empty() {
            return Err(ValidationError::EmptyBranchName);
        }
        Ok(Self {
            id,
            parent_id,
            session_id,
            name,
            status,
            created_at,
            completed_at,
            config,
        })
    }

    /// Returns the unique identifier of this branch.
    #[must_use]
    pub const fn id(&self) -> BranchId {
        self.id
    }

    /// Returns the parent branch ID if this is a child branch.
    #[must_use]
    pub const fn parent_id(&self) -> Option<BranchId> {
        self.parent_id
    }

    /// Returns the session ID associated with this branch.
    #[must_use]
    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    /// Returns the name of this branch.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the current status of this branch.
    #[must_use]
    pub const fn status(&self) -> super::status::BranchStatus {
        self.status
    }

    /// Returns the timestamp when this branch was created.
    #[must_use]
    pub const fn created_at(&self) -> i64 {
        self.created_at
    }

    /// Returns the timestamp when this branch was completed, if applicable.
    #[must_use]
    pub const fn completed_at(&self) -> Option<i64> {
        self.completed_at
    }

    /// Returns the configuration JSON string for this branch.
    #[must_use]
    pub fn config(&self) -> &str {
        &self.config
    }

    /// Checks if this branch is active (managed by supervisor).
    #[must_use]
    pub fn is_active(&self) -> bool {
        matches!(
            self.status,
            super::status::BranchStatus::Pending
                | super::status::BranchStatus::Active
                | super::status::BranchStatus::Merging
        )
    }
}

/// Execution strategy for running branch tasks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ExecutionStrategy {
    /// Execute tasks sequentially, one at a time.
    #[default]
    Sequential,
    /// Execute tasks in parallel with a maximum concurrency limit.
    Parallel {
        /// The maximum number of concurrent tasks allowed.
        max_concurrent: usize,
    },
}

impl ExecutionStrategy {
    /// Validates the execution strategy configuration.
    ///
    /// # Errors
    /// Returns `BranchValidationError::InvalidExecutionStrategy` if `max_concurrent` exceeds limit.
    pub fn validate(&self) -> Result<(), BranchValidationError> {
        match self {
            Self::Sequential => Ok(()),
            Self::Parallel { max_concurrent } => {
                if *max_concurrent == 0 {
                    Err(BranchValidationError::InvalidExecutionStrategy {
                        reason: "max_concurrent must be at least 1".to_string(),
                    })
                } else if *max_concurrent > MAX_CONCURRENT_BRANCHES {
                    Err(BranchValidationError::InvalidExecutionStrategy {
                        reason: format!("max_concurrent cannot exceed {MAX_CONCURRENT_BRANCHES}"),
                    })
                } else {
                    Ok(())
                }
            }
        }
    }

    /// Returns the effective concurrency limit.
    #[must_use]
    pub const fn concurrency_limit(&self) -> usize {
        match self {
            Self::Sequential => 1,
            Self::Parallel { max_concurrent } => *max_concurrent,
        }
    }
}

/// Assignment of an agent to a branch with optional overrides.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentAssignment {
    agent_id: AgentId,
    task_override: Option<String>,
    priority: Priority,
}

impl AgentAssignment {
    /// Creates a new agent assignment.
    ///
    /// # Errors
    /// Returns `ValidationError` if `agent_id` is empty.
    pub fn new(
        agent_id: impl Into<String>,
        task_override: Option<String>,
        priority: Priority,
    ) -> Result<Self, ValidationError> {
        Ok(Self {
            agent_id: AgentId::new(agent_id)?,
            task_override,
            priority,
        })
    }

    /// Returns the agent ID for this assignment.
    #[must_use]
    pub fn agent_id(&self) -> &AgentId {
        &self.agent_id
    }

    /// Returns the task override string if specified.
    #[must_use]
    pub fn task_override(&self) -> Option<&str> {
        self.task_override.as_deref()
    }

    /// Returns the priority level for this agent assignment.
    #[must_use]
    pub const fn priority(&self) -> Priority {
        self.priority
    }
}

/// Configuration for branch execution.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BranchConfig {
    name: String,
    agents: Vec<AgentAssignment>,
    execution_strategy: ExecutionStrategy,
    auto_merge: bool,
    merge_strategy: String,
}

impl BranchConfig {
    /// Creates a new branch configuration with validation.
    ///
    /// # Errors
    /// Returns `BranchValidationError` if validation fails.
    pub fn new(
        name: impl Into<String>,
        agents: Vec<AgentAssignment>,
        execution_strategy: ExecutionStrategy,
        auto_merge: bool,
        merge_strategy: impl Into<String>,
    ) -> Result<Self, BranchValidationError> {
        let name = name.into();
        let name_len = name.len();

        if !(MIN_BRANCH_NAME_LEN..=MAX_BRANCH_NAME_LEN).contains(&name_len) {
            return Err(BranchValidationError::InvalidNameLength {
                len: name_len,
                min: MIN_BRANCH_NAME_LEN,
                max: MAX_BRANCH_NAME_LEN,
            });
        }

        execution_strategy.validate()?;

        Ok(Self {
            name,
            agents,
            execution_strategy,
            auto_merge,
            merge_strategy: merge_strategy.into(),
        })
    }

    /// Returns the name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the agents.
    #[must_use]
    pub fn agents(&self) -> &[AgentAssignment] {
        &self.agents
    }

    /// Returns the execution strategy.
    #[must_use]
    pub const fn execution_strategy(&self) -> ExecutionStrategy {
        self.execution_strategy
    }

    /// Returns whether auto-merge is enabled.
    #[must_use]
    pub const fn auto_merge(&self) -> bool {
        self.auto_merge
    }

    /// Returns the merge strategy.
    #[must_use]
    pub fn merge_strategy(&self) -> &str {
        &self.merge_strategy
    }
}

impl Default for BranchConfig {
    fn default() -> Self {
        Self {
            name: "default-branch".to_string(),
            agents: Vec::new(),
            execution_strategy: ExecutionStrategy::default(),
            auto_merge: false,
            merge_strategy: "three-way".to_string(),
        }
    }
}
