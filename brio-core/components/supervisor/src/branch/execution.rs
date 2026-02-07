//! Parallel Execution Engine for Branch Orchestration
//!
//! Provides concurrent branch execution with resource limits, progress tracking,
//! and support for sequential and parallel agent execution strategies.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::{mpsc, Semaphore};

use crate::branch::manager::BranchManager;
use crate::domain::{
    AgentAssignment, AgentId, AgentResult, Branch, BranchConfig, BranchId, BranchResult,
    ExecutionMetrics, ExecutionStrategy, FileChange, Task, TaskId,
};
use crate::mesh_client::{AgentDispatcher, DispatchResult, MeshError};

/// Maximum time allowed for a single branch execution before timeout.
const DEFAULT_BRANCH_TIMEOUT: Duration = Duration::from_secs(300); // 5 minutes

/// Execution engine that runs branches in parallel with resource limits.
pub struct ParallelExecutionEngine {
    /// Reference to the branch manager for accessing branch state.
    branch_manager: Arc<BranchManager>,
    /// Dispatcher for sending tasks to agents.
    dispatcher: Arc<dyn AgentDispatcher + Send + Sync>,
    /// Maximum number of concurrent branches.
    max_concurrent: usize,
}

impl std::fmt::Debug for ParallelExecutionEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ParallelExecutionEngine")
            .field("branch_manager", &self.branch_manager)
            .field("max_concurrent", &self.max_concurrent)
            .finish_non_exhaustive()
    }
}

impl ParallelExecutionEngine {
    /// Creates a new parallel execution engine.
    ///
    /// # Panics
    /// Panics if `max_concurrent` exceeds `MAX_CONCURRENT_BRANCHES`.
    #[must_use]
    pub fn new(
        branch_manager: Arc<BranchManager>,
        dispatcher: Arc<dyn AgentDispatcher + Send + Sync>,
        max_concurrent: usize,
    ) -> Self {
        assert!(
            max_concurrent <= 8,
            "max_concurrent cannot exceed 8",
        );

        Self {
            branch_manager,
            dispatcher,
            max_concurrent,
        }
    }

    /// Executes a single branch.
    ///
    /// # Errors
    /// Returns `ExecutionError` if:
    /// - Branch is not found
    /// - Status transition is invalid
    /// - Agent execution fails
    /// - Execution times out
    pub async fn execute_branch(
        &self,
        branch_id: BranchId,
    ) -> Result<BranchResult, ExecutionError> {
        self.execute_branch_with_timeout(branch_id, DEFAULT_BRANCH_TIMEOUT)
            .await
    }

    /// Executes a single branch with a custom timeout.
    ///
    /// # Errors
    /// Returns `ExecutionError` if execution fails or times out.
    pub async fn execute_branch_with_timeout(
        &self,
        branch_id: BranchId,
        timeout: Duration,
    ) -> Result<BranchResult, ExecutionError> {
        let start_time = Instant::now();

        // 1. Get branch from manager
        let branch = self
            .branch_manager
            .get_branch(branch_id)
            .map_err(|e| ExecutionError::Branch(e.to_string()))?
            .ok_or_else(|| ExecutionError::Branch(format!("Branch {} not found", branch_id)))?;

        // 2. Parse branch config from JSON
        let config: BranchConfig = serde_json::from_str(branch.config())
            .map_err(|e| ExecutionError::Branch(format!("Failed to parse config: {}", e)))?;

        // 3. Update status to Executing
        let total_agents = config.agents().len();
        self.branch_manager
            .mark_executing(branch_id, total_agents)
            .map_err(|e| ExecutionError::Branch(e.to_string()))?;

        // 4. Execute agents based on strategy
        let agent_results = match config.execution_strategy() {
            ExecutionStrategy::Sequential => {
                self.execute_agents_sequential(&branch, &config).await?
            }
            ExecutionStrategy::Parallel { max_concurrent } => {
                self.execute_agents_parallel(&branch, &config, max_concurrent)
                    .await?
            }
        };

        // 5. Collect file changes (would come from VFS session in real implementation)
        let file_changes = self.collect_file_changes(&branch, &agent_results).await?;

        // 6. Create BranchResult
        let total_duration = start_time.elapsed();
        let metrics = ExecutionMetrics {
            total_duration_ms: total_duration.as_millis() as u64,
            files_processed: file_changes.len(),
            agents_executed: agent_results.len(),
            peak_memory_bytes: 0, // Would be populated from actual measurements
        };

        let result = BranchResult::new(
            branch_id,
            file_changes,
            agent_results,
            metrics,
        );

        // 7. Mark branch as Completed
        self.branch_manager
            .complete_branch(branch_id, result.clone())
            .map_err(|e| ExecutionError::Branch(e.to_string()))?;

        // Check timeout
        if total_duration > timeout {
            return Err(ExecutionError::Timeout {
                branch_id,
                duration: total_duration,
            });
        }

        Ok(result)
    }

    /// Executes agents sequentially, one at a time.
    async fn execute_agents_sequential(
        &self,
        branch: &Branch,
        config: &BranchConfig,
    ) -> Result<Vec<AgentResult>, ExecutionError> {
        let mut results = Vec::with_capacity(config.agents().len());

        for (idx, assignment) in config.agents().iter().enumerate() {
            // Update progress
            self.branch_manager
                .update_progress(branch.id(), 1, idx)
                .map_err(|e| ExecutionError::Branch(e.to_string()))?;

            let session_path = PathBuf::from(branch.session_id());
            let result = execute_agent_on_branch(
                self.dispatcher.as_ref(),
                branch,
                assignment,
                &session_path,
            )
            .await?;

            results.push(result);
        }

        Ok(results)
    }

    /// Executes agents in parallel with a concurrency limit.
    async fn execute_agents_parallel(
        &self,
        branch: &Branch,
        config: &BranchConfig,
        max_concurrent: usize,
    ) -> Result<Vec<AgentResult>, ExecutionError> {
        let semaphore = Arc::new(Semaphore::new(max_concurrent));
        let dispatcher = self.dispatcher.clone();
        let session_path = PathBuf::from(branch.session_id());

        let mut handles = Vec::with_capacity(config.agents().len());

        for (idx, assignment) in config.agents().iter().enumerate() {
            let permit = semaphore
                .clone()
                .acquire_owned()
                .await
                .map_err(|e| ExecutionError::Branch(format!("Semaphore error: {}", e)))?;
            let dispatcher = dispatcher.clone();
            let branch = branch.clone();
            let assignment = assignment.clone();
            let session_path = session_path.clone();
            let branch_manager = self.branch_manager.clone();

            let handle = tokio::spawn(async move {
                let _permit = permit; // Hold permit until task completes

                // Update progress before execution
                let _ = branch_manager.update_progress(branch.id(), 1, idx);

                execute_agent_on_branch(dispatcher.as_ref(), &branch, &assignment, &session_path)
                    .await
            });

            handles.push(handle);
        }

        let mut results = Vec::with_capacity(handles.len());
        for handle in handles {
            match handle.await {
                Ok(result) => results.push(result?),
                Err(e) => {
                    return Err(ExecutionError::Branch(format!(
                        "Agent task panicked: {}",
                        e
                    )))
                }
            }
        }

        Ok(results)
    }

    /// Collects file changes from the branch session.
    async fn collect_file_changes(
        &self,
        _branch: &Branch,
        _agent_results: &[AgentResult],
    ) -> Result<Vec<FileChange>, ExecutionError> {
        // In a real implementation, this would query the VFS session
        // for files modified by the agents.
        Ok(Vec::new())
    }

    /// Executes multiple branches in parallel with a semaphore limit.
    ///
    /// Returns results in the same order as the input branch IDs.
    ///
    /// # Errors
    /// Returns individual `ExecutionError` for each branch that fails.
    pub async fn execute_branches_parallel(
        &self,
        branches: Vec<BranchId>,
    ) -> Vec<Result<BranchResult, ExecutionError>> {
        let semaphore = Arc::new(Semaphore::new(self.max_concurrent));
        let mut handles = Vec::with_capacity(branches.len());

        for branch_id in branches {
            let permit = match semaphore.clone().try_acquire_owned() {
                Ok(p) => p,
                Err(_) => {
                    // If semaphore is full, acquire asynchronously
                    let sem = semaphore.clone();
                    match sem.acquire_owned().await {
                        Ok(p) => p,
                        Err(e) => {
                            handles.push(tokio::spawn(async move {
                                Err(ExecutionError::Branch(format!(
                                    "Failed to acquire semaphore: {}",
                                    e
                                )))
                            }));
                            continue;
                        }
                    }
                }
            };

            let engine = Self {
                branch_manager: self.branch_manager.clone(),
                dispatcher: self.dispatcher.clone(),
                max_concurrent: self.max_concurrent,
            };

            let handle = tokio::spawn(async move {
                let _permit = permit; // Hold permit until done
                engine.execute_branch(branch_id).await
            });

            handles.push(handle);
        }

        let mut results = Vec::with_capacity(handles.len());
        for handle in handles {
            match handle.await {
                Ok(result) => results.push(result),
                Err(e) => results.push(Err(ExecutionError::Branch(format!(
                    "Task join error: {}",
                    e
                )))),
            }
        }

        results
    }

    /// Executes a tree of branches (root + nested children).
    ///
    /// # Errors
    /// Returns `ExecutionError` if any branch in the tree fails.
    pub async fn execute_branch_tree(
        &self,
        root_id: BranchId,
    ) -> Result<BranchTreeResult, ExecutionError> {
        let start_time = Instant::now();

        // 1. Execute root branch first
        let root_result = self.execute_branch(root_id).await?;

        // 2. Get all children from the branch tree
        let branch_tree = self
            .branch_manager
            .get_branch_tree(root_id)
            .map_err(|e| ExecutionError::Branch(e.to_string()))?;

        // 3. Execute children in parallel
        let child_results = if branch_tree.children.is_empty() {
            Vec::new()
        } else {
            let children_ids: Vec<BranchId> = branch_tree
                .children
                .iter()
                .map(|child| child.branch.id())
                .collect();

            let results = self.execute_branches_parallel(children_ids.clone()).await;

            // Collect successful results, fail if any child failed
            let mut successful = Vec::new();
            for (idx, result) in results.into_iter().enumerate() {
                match result {
                    Ok(_branch_result) => {
                        // Recursively execute child's children (boxed to avoid infinite-sized type)
                        let child_id = children_ids[idx];
                        let tree_result = Box::pin(self.execute_branch_tree(child_id)).await?;
                        successful.push(tree_result);
                    }
                    Err(e) => return Err(e),
                }
            }
            successful
        };

        // 4. Compute totals
        let total_agents = root_result.metrics().agents_executed
            + child_results
                .iter()
                .map(|r| r.total_agents_executed)
                .sum::<usize>();

        let total_duration = start_time.elapsed();

        // 5. Auto-merge if enabled
        let branch = self
            .branch_manager
            .get_branch(root_id)
            .map_err(|e| ExecutionError::Branch(e.to_string()))?
            .ok_or_else(|| ExecutionError::Branch(format!("Branch {} not found", root_id)))?;

        // Parse config to check auto_merge
        let config: BranchConfig = serde_json::from_str(branch.config())
            .map_err(|e| ExecutionError::Branch(format!("Failed to parse config: {}", e)))?;
        
        if config.auto_merge() && !child_results.is_empty() {
            // Merge children results into parent
            self.merge_children_into_parent(root_id, &child_results)
                .await?;
        }

        Ok(BranchTreeResult {
            root: root_result,
            children: child_results,
            total_execution_time: total_duration,
            total_agents_executed: total_agents,
        })
    }

    /// Merges child branch results into a parent branch.
    async fn merge_children_into_parent(
        &self,
        _parent_id: BranchId,
        _child_results: &[BranchTreeResult],
    ) -> Result<(), ExecutionError> {
        // In a real implementation, this would invoke the merge logic
        // from the merge.rs module.
        Ok(())
    }

    /// Executes a branch with progress tracking via channel.
    ///
    /// # Errors
    /// Returns `ExecutionError` if execution fails or channel send fails.
    pub async fn execute_with_progress(
        &self,
        branch_id: BranchId,
        progress_tx: mpsc::Sender<BranchProgress>,
    ) -> Result<BranchResult, ExecutionError> {
        let start_time = Instant::now();

        // Get branch
        let branch = self
            .branch_manager
            .get_branch(branch_id)
            .map_err(|e| ExecutionError::Branch(e.to_string()))?
            .ok_or_else(|| ExecutionError::Branch(format!("Branch {} not found", branch_id)))?;

        // Parse config
        let config: BranchConfig = serde_json::from_str(branch.config())
            .map_err(|e| ExecutionError::Branch(format!("Failed to parse config: {}", e)))?;
        let total_agents = config.agents().len();

        // Send initial progress
        let _ = progress_tx
            .send(BranchProgress {
                branch_id,
                total_agents,
                completed_agents: 0,
                current_agent: None,
                percent_complete: 0.0,
                status: ExecutionStatus::Pending,
            })
            .await;

        // Start execution
        self.branch_manager
            .mark_executing(branch_id, total_agents)
            .map_err(|e| ExecutionError::Branch(e.to_string()))?;

        // Execute agents with progress updates
        let mut agent_results = Vec::with_capacity(total_agents);
        let session_path = PathBuf::from(branch.session_id());

        for (idx, assignment) in config.agents().iter().enumerate() {
            // Send progress before starting agent
            let _ = progress_tx
                .send(BranchProgress {
                    branch_id,
                    total_agents,
                    completed_agents: idx,
                    current_agent: Some(assignment.agent_id().clone()),
                    percent_complete: (idx as f32 / total_agents as f32) * 100.0,
                    status: ExecutionStatus::Executing {
                        active: 1,
                        completed: idx,
                    },
                })
                .await;

            let result = execute_agent_on_branch(
                self.dispatcher.as_ref(),
                &branch,
                assignment,
                &session_path,
            )
            .await?;

            agent_results.push(result);
        }

        // Collect results
        let file_changes = self.collect_file_changes(&branch, &agent_results).await?;

        let total_duration = start_time.elapsed();
        let metrics = ExecutionMetrics {
            total_duration_ms: total_duration.as_millis() as u64,
            files_processed: file_changes.len(),
            agents_executed: agent_results.len(),
            peak_memory_bytes: 0,
        };

        let result = BranchResult::new(
            branch_id,
            file_changes,
            agent_results,
            metrics,
        );

        // Mark as completed
        self.branch_manager
            .complete_branch(branch_id, result.clone())
            .map_err(|e| ExecutionError::Branch(e.to_string()))?;

        // Send final progress
        let _ = progress_tx
            .send(BranchProgress {
                branch_id,
                total_agents,
                completed_agents: total_agents,
                current_agent: None,
                percent_complete: 100.0,
                status: ExecutionStatus::Completed,
            })
            .await;

        Ok(result)
    }

    /// Cancels execution of a branch.
    ///
    /// # Errors
    /// Returns `ExecutionError` if cancellation fails.
    pub async fn cancel_execution(
        &self,
        branch_id: BranchId,
    ) -> Result<(), ExecutionError> {
        // In a real implementation, this would:
        // 1. Signal cancellation to running agents
        // 2. Wait for graceful shutdown
        // 3. Rollback the branch

        self.branch_manager
            .abort_branch(branch_id)
            .await
            .map_err(|e| ExecutionError::Branch(e.to_string()))?;

        Ok(())
    }
}

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

/// Executes an agent on a branch.
///
/// # Errors
/// Returns `ExecutionError` if dispatch fails or agent returns an error.
async fn execute_agent_on_branch(
    dispatcher: &dyn AgentDispatcher,
    branch: &Branch,
    assignment: &AgentAssignment,
    _session_path: &Path,
) -> Result<AgentResult, ExecutionError> {
    let start_time = Instant::now();

    // Create task for the agent
    let task_content = assignment
        .task_override()
        .map(|s| s.to_string())
        .unwrap_or_else(|| format!("Execute on branch {}", branch.name()));

    let task = Task::new(
        TaskId::new(0), // Would generate proper ID in real implementation
        task_content,
        assignment.priority(),
        crate::domain::TaskStatus::Pending,
        None,
        Some(assignment.agent_id().clone()),
        std::collections::HashSet::new(),
    )
    .map_err(|e| ExecutionError::Branch(format!("Failed to create task: {}", e)))?;

    // Dispatch to agent
    let dispatch_result = dispatcher
        .dispatch(assignment.agent_id(), &task)
        .map_err(ExecutionError::Dispatch)?;

    let duration = start_time.elapsed();

    match dispatch_result {
        DispatchResult::Completed(output) => Ok(AgentResult {
            agent_id: assignment.agent_id().clone(),
            success: true,
            output: Some(output),
            error: None,
            duration_ms: duration.as_millis() as u64,
        }),
        DispatchResult::Accepted => Ok(AgentResult {
            agent_id: assignment.agent_id().clone(),
            success: true,
            output: None,
            error: None,
            duration_ms: duration.as_millis() as u64,
        }),
        DispatchResult::AgentBusy => Err(ExecutionError::AgentFailed {
            agent_id: assignment.agent_id().clone(),
            reason: "Agent is busy".to_string(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::Priority;

    /// Mock dispatcher for testing.
    #[derive(Debug)]
    struct MockDispatcher {
        should_succeed: bool,
    }

    impl AgentDispatcher for MockDispatcher {
        fn dispatch(
            &self,
            agent: &AgentId,
            _task: &Task,
        ) -> Result<DispatchResult, MeshError> {
            if self.should_succeed {
                Ok(DispatchResult::Completed(format!(
                    "Agent {} completed",
                    agent.as_str()
                )))
            } else {
                Err(MeshError::AgentError("Agent failed".to_string()))
            }
        }
    }

    // Note: These tests require a fully initialized BranchManager with SessionManager
    // and BranchRepository, which requires filesystem and database access.
    // For unit tests, we would need to create mock implementations.

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
