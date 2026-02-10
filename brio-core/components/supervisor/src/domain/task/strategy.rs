//! Task domain - Branching strategy detection
//!
//! This module defines branching-related types and strategy detection.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// The source from which to create a branch.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum BranchSource {
    /// Branch from the base workspace at the given path.
    Base(PathBuf),
    /// Branch from an existing branch.
    Branch(super::super::ids::BranchId),
}

/// Strategy for branching task execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BranchingStrategy {
    /// Multiple reviewers for code review from different perspectives.
    MultipleReviewers,
    /// Alternative implementations to compare approaches (A/B testing).
    AlternativeImplementations,
    /// Nested branches for complex refactors with sub-tasks.
    NestedBranches,
}

/// Analyzes task content to determine if branching is needed.
///
/// Returns `Some(BranchingStrategy)` if the task content indicates
/// that branching execution would be beneficial, or `None` if the
/// task should proceed with standard single-path execution.
#[must_use]
pub fn should_use_branching(task: &super::entities::Task) -> Option<BranchingStrategy> {
    let content = task.content().to_lowercase();

    if content.contains("multiple reviewers")
        || content.contains("security and performance review")
        || content.contains("code review from different perspectives")
    {
        Some(BranchingStrategy::MultipleReviewers)
    } else if content.contains("implement both")
        || content.contains("a/b test")
        || content.contains("compare approaches")
        || content.contains("alternative implementations")
    {
        Some(BranchingStrategy::AlternativeImplementations)
    } else if content.contains("refactor") && content.contains("sub-tasks") {
        Some(BranchingStrategy::NestedBranches)
    } else {
        None
    }
}

/// Capabilities that an agent can possess or a task can require.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Capability {
    /// Ability to generate or modify code.
    Coding,
    /// Ability to review code or designs.
    Reviewing,
    /// Ability to reason about system architecture.
    Reasoning,
}

impl core::fmt::Display for Capability {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Coding => write!(f, "Coding"),
            Self::Reviewing => write!(f, "Reviewing"),
            Self::Reasoning => write!(f, "Reasoning"),
        }
    }
}
