//! Progress tracking and error types for branch execution.
//!
//! Provides types for monitoring execution progress, timeouts, and errors.

use std::time::Duration;

use crate::domain::{AgentId, BranchId, BranchResult};
use crate::mesh_client::MeshError;

/// Execution status for progress tracking.
#[derive(Debug, Clone)]
pub enum ExecutionStatus {
    /// Initial pending state.
    Pending,
    /// Currently executing with progress info.
    Executing {
        /// Number of agents currently active.
        active: usize,
        /// Number of agents completed so far.
        completed: usize,
    },
    /// Execution completed successfully.
    Completed,
    /// Execution failed.
    Failed(String),
}

/// Progress updates during branch execution.
#[derive(Debug, Clone)]
pub struct BranchProgress {
    /// The branch being executed.
    pub branch_id: BranchId,
    /// Total number of agents to execute.
    pub total_agents: usize,
    /// Number of agents completed so far.
    pub completed_agents: usize,
    /// Currently executing agent, if any.
    pub current_agent: Option<AgentId>,
    /// Percentage complete (0.0 - 100.0).
    pub percent_complete: f32,
    /// Current execution status.
    pub status: ExecutionStatus,
}

/// Result of executing a branch tree.
#[derive(Debug)]
pub struct BranchTreeResult {
    /// Result from the root branch.
    pub root: BranchResult,
    /// Results from child branches.
    pub children: Vec<BranchTreeResult>,
    /// Total execution time for the entire tree.
    pub total_execution_time: Duration,
    /// Total number of agents executed across all branches.
    pub total_agents_executed: usize,
}

/// Errors that can occur during branch execution.
#[derive(Debug, thiserror::Error)]
pub enum ExecutionError {
    /// Branch-related error.
    #[error("Branch error: {0}")]
    Branch(String),
    /// Dispatch error from mesh client.
    #[error("Dispatch error: {0}")]
    Dispatch(#[from] MeshError),
    /// Agent execution failed.
    #[error("Agent {agent_id} failed: {reason}")]
    AgentFailed {
        /// The agent that failed.
        agent_id: AgentId,
        /// Reason for failure.
        reason: String,
    },
    /// Execution was cancelled.
    #[error("Execution was cancelled")]
    Cancelled,
    /// Execution timed out.
    #[error("Branch {branch_id} timed out after {duration:?}")]
    Timeout {
        /// The branch that timed out.
        branch_id: BranchId,
        /// Duration before timeout.
        duration: Duration,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{AgentId, BranchId, BranchResult, ExecutionMetrics};
    use crate::mesh_client::MeshError;

    #[test]
    fn test_execution_error_display() {
        let id = BranchId::from_uuid(uuid::Uuid::from_u128(1));

        let err = ExecutionError::Branch("test error".to_string());
        assert!(err.to_string().contains("test error"));

        let err = ExecutionError::Timeout {
            branch_id: id,
            duration: Duration::from_secs(60),
        };
        assert!(err.to_string().contains("timed out"));

        let agent_id = AgentId::new("test-agent").unwrap();
        let err = ExecutionError::AgentFailed {
            agent_id,
            reason: "crashed".to_string(),
        };
        assert!(err.to_string().contains("crashed"));
    }

    #[test]
    fn test_execution_error_from_mesh_error() {
        let mesh_err = MeshError::AgentNotFound("agent-1".to_string());
        let exec_err: ExecutionError = mesh_err.into();
        assert!(matches!(exec_err, ExecutionError::Dispatch(_)));
    }

    #[test]
    fn test_branch_progress_creation() {
        let branch_id = BranchId::from_uuid(uuid::Uuid::from_u128(1));
        let progress = BranchProgress {
            branch_id,
            total_agents: 5,
            completed_agents: 2,
            current_agent: Some(AgentId::new("agent-1").unwrap()),
            percent_complete: 40.0,
            status: ExecutionStatus::Executing {
                active: 1,
                completed: 2,
            },
        };

        assert_eq!(progress.branch_id, branch_id);
        assert_eq!(progress.total_agents, 5);
        assert_eq!(progress.completed_agents, 2);
        assert!((progress.percent_complete - 40.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_branch_tree_result() {
        let root_result = BranchResult {
            branch_id: BranchId::from_uuid(uuid::Uuid::from_u128(1)),
            file_changes: vec![],
            agent_results: vec![],
            metrics: ExecutionMetrics {
                total_duration_ms: 1000,
                files_processed: 5,
                agents_executed: 3,
                peak_memory_bytes: 1024,
            },
        };
        let tree_result = BranchTreeResult {
            root: root_result.clone(),
            children: vec![],
            total_execution_time: Duration::from_secs(1),
            total_agents_executed: 3,
        };

        assert_eq!(tree_result.total_agents_executed, 3);
        assert!(tree_result.children.is_empty());
    }
}
