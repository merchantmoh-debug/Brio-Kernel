//! Core execution engine for branch orchestration.
//!
//! Provides the main ParallelExecutionEngine that manages branch execution
//! with support for timeouts, parallel execution, and tree execution.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::{mpsc, Semaphore};

use crate::branch::coordinator::BranchManager;
use crate::branch::Branch;
use crate::domain::{
    AgentAssignment, AgentId, AgentResult, BranchConfig, BranchId, BranchResult,
    ExecutionMetrics, ExecutionStrategy, FileChange, Task, TaskId,
};
use crate::mesh_client::AgentDispatcher;

use super::strategies::{execute_agents_parallel, execute_agents_sequential};
use super::tracking::{
    BranchProgress, BranchTreeResult, ExecutionError, ExecutionStatus,
};

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

        // 2. Get branch config directly from branch entity
        let config = branch.config();

        // 3. Update status to Executing
        let total_agents = config.agents().len();
        self.branch_manager
            .mark_executing(branch_id, total_agents)
            .map_err(|e| ExecutionError::Branch(e.to_string()))?;

        // 4. Execute agents based on strategy
        let agent_results = match config.execution_strategy() {
            ExecutionStrategy::Sequential => {
                execute_agents_sequential(
                    self.branch_manager.clone(),
                    self.dispatcher.clone(),
                    &branch,
                    &config,
                )
                .await?
            }
            ExecutionStrategy::Parallel { max_concurrent } => {
                execute_agents_parallel(
                    self.branch_manager.clone(),
                    self.dispatcher.clone(),
                    &branch,
                    &config,
                    max_concurrent,
                )
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

    /// Collects file changes from the branch session.
    async fn collect_file_changes(
        &self,
        _branch: &Branch,
        _agent_results: &[crate::domain::AgentResult],
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

        // Check auto_merge flag from config
        if branch.config().auto_merge() && !child_results.is_empty() {
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

        // Get config and total agents
        let config = branch.config();
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

            let result = super::strategies::execute_agent_on_branch(
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
