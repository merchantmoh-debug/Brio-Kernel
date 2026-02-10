//! Change types for merge operations.
//!
//! This module defines types for tracking file changes during merge operations.

use crate::domain::ids::{AgentId, BranchId};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Type of file change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChangeType {
    /// File was added.
    Added,
    /// File was modified.
    Modified,
    /// File was deleted.
    Deleted,
    /// File was renamed.
    Renamed,
}

/// A single file change.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FileChange {
    path: PathBuf,
    change_type: ChangeType,
    diff: Option<String>,
}

impl FileChange {
    /// Creates a new file change.
    #[must_use]
    pub fn new(path: PathBuf, change_type: ChangeType, diff: Option<String>) -> Self {
        Self {
            path,
            change_type,
            diff,
        }
    }

    /// Returns the path.
    #[must_use]
    pub fn path(&self) -> &PathBuf {
        &self.path
    }

    /// Returns the change type.
    #[must_use]
    pub const fn change_type(&self) -> ChangeType {
        self.change_type
    }

    /// Returns the diff.
    #[must_use]
    pub fn diff(&self) -> Option<&str> {
        self.diff.as_deref()
    }
}

/// A file change in the staging area.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StagedChange {
    /// Path of the file.
    pub path: PathBuf,
    /// Type of change.
    pub change_type: ChangeType,
    /// Content hash for verification.
    pub content_hash: Option<String>,
}

/// Execution metrics for a branch.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ExecutionMetrics {
    /// Total execution time in milliseconds.
    pub total_duration_ms: u64,
    /// Number of files processed.
    pub files_processed: usize,
    /// Number of agents executed.
    pub agents_executed: usize,
    /// Peak memory usage in bytes.
    pub peak_memory_bytes: u64,
}

/// Result from a single agent execution.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentResult {
    /// The agent that executed.
    pub agent_id: AgentId,
    /// Whether the execution succeeded.
    pub success: bool,
    /// Output from the agent.
    pub output: Option<String>,
    /// Error message if failed.
    pub error: Option<String>,
    /// Execution duration.
    pub duration_ms: u64,
}

/// Result of branch execution.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BranchResult {
    pub(crate) branch_id: BranchId,
    pub(crate) file_changes: Vec<FileChange>,
    pub(crate) agent_results: Vec<AgentResult>,
    pub(crate) metrics: ExecutionMetrics,
}

impl BranchResult {
    /// Creates a new branch result.
    #[must_use]
    pub fn new(
        branch_id: BranchId,
        file_changes: Vec<FileChange>,
        agent_results: Vec<AgentResult>,
        metrics: ExecutionMetrics,
    ) -> Self {
        Self {
            branch_id,
            file_changes,
            agent_results,
            metrics,
        }
    }

    /// Returns the branch ID.
    #[must_use]
    pub fn branch_id(&self) -> BranchId {
        self.branch_id
    }

    /// Returns the file changes.
    #[must_use]
    pub fn file_changes(&self) -> &[FileChange] {
        &self.file_changes
    }

    /// Returns the agent results.
    #[must_use]
    pub fn agent_results(&self) -> &[AgentResult] {
        &self.agent_results
    }

    /// Returns the metrics.
    #[must_use]
    pub fn metrics(&self) -> &ExecutionMetrics {
        &self.metrics
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_change_creation() {
        let change = FileChange::new(
            PathBuf::from("src/main.rs"),
            ChangeType::Modified,
            Some("diff content".to_string()),
        );

        assert_eq!(change.path(), &PathBuf::from("src/main.rs"));
        assert!(matches!(change.change_type(), ChangeType::Modified));
        assert_eq!(change.diff(), Some("diff content"));
    }

    #[test]
    fn test_change_type_variants() {
        assert!(matches!(ChangeType::Added, ChangeType::Added));
        assert!(matches!(ChangeType::Modified, ChangeType::Modified));
        assert!(matches!(ChangeType::Deleted, ChangeType::Deleted));
        assert!(matches!(ChangeType::Renamed, ChangeType::Renamed));
    }

    #[test]
    fn test_execution_metrics() {
        let metrics = ExecutionMetrics {
            total_duration_ms: 1000,
            files_processed: 5,
            agents_executed: 2,
            peak_memory_bytes: 1024 * 1024,
        };

        assert_eq!(metrics.total_duration_ms, 1000);
        assert_eq!(metrics.files_processed, 5);
        assert_eq!(metrics.agents_executed, 2);
        assert_eq!(metrics.peak_memory_bytes, 1024 * 1024);
    }

    #[test]
    fn test_agent_result() {
        let agent_id = AgentId::new("test-agent").unwrap();
        let result = AgentResult {
            agent_id,
            success: true,
            output: Some("success".to_string()),
            error: None,
            duration_ms: 500,
        };

        assert!(result.success);
        assert_eq!(result.duration_ms, 500);
        assert_eq!(result.output, Some("success".to_string()));
    }

    #[test]
    fn test_staged_change() {
        let staged = StagedChange {
            path: PathBuf::from("file.txt"),
            change_type: ChangeType::Added,
            content_hash: Some("abc123".to_string()),
        };

        assert_eq!(staged.path, PathBuf::from("file.txt"));
        assert!(matches!(staged.change_type, ChangeType::Added));
        assert_eq!(staged.content_hash, Some("abc123".to_string()));
    }
}
