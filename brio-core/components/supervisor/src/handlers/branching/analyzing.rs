//! Handler for `AnalyzingForBranch` state.
//!
//! Analyzes task content to determine if branching is needed and creates
//! branches if applicable.

use crate::domain::{BranchConfig, BranchId, BranchingStrategy, Task, TaskStatus};
use crate::handlers::branching::BranchManagerError;
use crate::handlers::{BranchManager, SupervisorContext, TaskStateHandler};
use crate::orchestrator::{Planner, SupervisorError};
use crate::repository::TaskRepository;

/// Merge strategy constants to avoid magic strings.
pub mod merge_strategy {
    /// Three-way merge strategy.
    pub const THREE_WAY: &str = "three-way";
    /// Union merge strategy.
    pub const UNION: &str = "union";
    /// Ours merge strategy.
    pub const OURS: &str = "ours";
    /// Theirs merge strategy.
    pub const THEIRS: &str = "theirs";
}

/// Handler for `AnalyzingForBranch` state.
///
/// Analyzes task content to determine if branching is needed and creates
/// branches if applicable.
pub struct AnalyzingForBranchHandler;

impl<R, D, P, S> TaskStateHandler<R, D, P, S> for AnalyzingForBranchHandler
where
    R: TaskRepository,
    D: crate::mesh_client::AgentDispatcher,
    P: Planner,
    S: crate::selector::AgentSelector,
{
    fn handle(
        &self,
        ctx: &SupervisorContext<R, D, P, S>,
        task: &Task,
    ) -> Result<bool, SupervisorError> {
        // Check if branch manager is available
        let Some(branch_manager) = ctx.branch_manager.as_ref() else {
            // No branch manager configured, proceed to Executing
            ctx.repository
                .update_status(task.id(), TaskStatus::Executing)
                .map_err(SupervisorError::StatusUpdateFailure)?;
            return Ok(true);
        };

        // Analyze task to determine branching strategy
        if let Some(strategy) = crate::domain::should_use_branching(task) {
            // Create branches based on strategy
            let branches = create_branches_for_strategy(branch_manager.as_ref(), task, strategy)
                .map_err(|e| {
                    SupervisorError::RepositoryFailure(
                        crate::repository::RepositoryError::SqlError(e.to_string()),
                    )
                })?;

            if branches.is_empty() {
                // No branches created, fall back to Executing
                ctx.repository
                    .update_status(task.id(), TaskStatus::Executing)
                    .map_err(SupervisorError::StatusUpdateFailure)?;
                Ok(true)
            } else {
                // Transition to Branching state
                ctx.repository
                    .update_status(
                        task.id(),
                        TaskStatus::Branching {
                            branches: branches.clone(),
                            completed: 0,
                            total: branches.len(),
                        },
                    )
                    .map_err(SupervisorError::StatusUpdateFailure)?;

                // Start executing branches
                branch_manager.execute_branches(&branches).map_err(|e| {
                    SupervisorError::RepositoryFailure(
                        crate::repository::RepositoryError::SqlError(e.to_string()),
                    )
                })?;

                Ok(true)
            }
        } else {
            // No branching needed, transition to Executing
            ctx.repository
                .update_status(task.id(), TaskStatus::Executing)
                .map_err(SupervisorError::StatusUpdateFailure)?;
            Ok(true)
        }
    }
}

/// Creates branches based on the branching strategy.
fn create_branches_for_strategy(
    branch_manager: &dyn BranchManager,
    task: &Task,
    strategy: BranchingStrategy,
) -> Result<Vec<BranchId>, BranchManagerError> {
    let mut branches = Vec::new();

    match strategy {
        BranchingStrategy::MultipleReviewers => {
            // Create branches for different reviewer perspectives
            let reviewers = vec!["security-reviewer", "performance-reviewer", "code-reviewer"];
            for reviewer in reviewers {
                let config = BranchConfig::new(
                    format!("{}-{}", task.id(), reviewer),
                    vec![],
                    crate::domain::ExecutionStrategy::Sequential,
                    false,
                    merge_strategy::THREE_WAY,
                )
                .map_err(|e| {
                    BranchManagerError::CreateError(format!("Failed to create branch config: {e}"))
                })?;

                let branch_id =
                    branch_manager.create_branch(None, config.name().to_string(), config)?;
                branches.push(branch_id);
            }
        }
        BranchingStrategy::AlternativeImplementations => {
            // Create branches for alternative approaches
            let approaches = vec!["approach-a", "approach-b"];
            for approach in approaches {
                let config = BranchConfig::new(
                    format!("{}-{}", task.id(), approach),
                    vec![],
                    crate::domain::ExecutionStrategy::Sequential,
                    false,
                    merge_strategy::THREE_WAY,
                )
                .map_err(|e| {
                    BranchManagerError::CreateError(format!("Failed to create branch config: {e}"))
                })?;

                let branch_id =
                    branch_manager.create_branch(None, config.name().to_string(), config)?;
                branches.push(branch_id);
            }
        }
        BranchingStrategy::NestedBranches => {
            // Create branches for sub-tasks
            // For now, create a single branch - in practice, this would parse
            // the task content to identify sub-tasks
            let config = BranchConfig::new(
                format!("{}-refactor", task.id()),
                vec![],
                crate::domain::ExecutionStrategy::Sequential,
                false,
                merge_strategy::THREE_WAY,
            )
            .map_err(|e| {
                BranchManagerError::CreateError(format!("Failed to create branch config: {e}"))
            })?;

            let branch_id =
                branch_manager.create_branch(None, config.name().to_string(), config)?;
            branches.push(branch_id);
        }
    }

    Ok(branches)
}
