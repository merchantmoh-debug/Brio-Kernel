//! Execution strategies for agent tasks.
//!
//! Provides sequential and parallel execution of agents within a branch.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use tokio::sync::Semaphore;

use crate::branch::manager::BranchManager;
use crate::branch::Branch;
use crate::domain::{
    AgentAssignment, AgentId, AgentResult, BranchConfig, BranchId,
    ExecutionStrategy, Task, TaskId,
};
use crate::mesh_client::{AgentDispatcher, DispatchResult, MeshError};

use super::tracking::ExecutionError;

/// Sequential execution strategy - executes agents one at a time.
pub async fn execute_agents_sequential(
    branch_manager: Arc<BranchManager>,
    dispatcher: Arc<dyn AgentDispatcher + Send + Sync>,
    branch: &Branch,
    config: &BranchConfig,
) -> Result<Vec<AgentResult>, ExecutionError> {
    let mut results = Vec::with_capacity(config.agents().len());

    for (idx, assignment) in config.agents().iter().enumerate() {
        // Update progress
        branch_manager
            .update_progress(branch.id(), 1, idx)
            .map_err(|e| ExecutionError::Branch(e.to_string()))?;

        let session_path = PathBuf::from(branch.session_id());
        let result = execute_agent_on_branch(
            dispatcher.as_ref(),
            branch,
            assignment,
            &session_path,
        )
        .await?;

        results.push(result);
    }

    Ok(results)
}

/// Parallel execution strategy - executes agents concurrently with a limit.
pub async fn execute_agents_parallel(
    branch_manager: Arc<BranchManager>,
    dispatcher: Arc<dyn AgentDispatcher + Send + Sync>,
    branch: &Branch,
    config: &BranchConfig,
    max_concurrent: usize,
) -> Result<Vec<AgentResult>, ExecutionError> {
    let semaphore = Arc::new(Semaphore::new(max_concurrent));
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
        let branch_manager = branch_manager.clone();

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

/// Dispatches an agent task to be executed on a branch.
///
/// # Errors
/// Returns `ExecutionError` if dispatch fails or agent returns an error.
pub async fn execute_agent_on_branch(
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
