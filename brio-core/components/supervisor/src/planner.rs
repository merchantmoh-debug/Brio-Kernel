//! Planner Layer - Task Decomposition
//!
//! Abstracts the planner capability via the WIT `planner` interface.

use crate::orchestrator::{Planner, PlannerError};
use crate::wit_bindings;

/// Planner implementation using WIT `planner` bindings.
pub struct WitPlanner;

impl WitPlanner {
    /// Creates a new WIT-backed planner.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for WitPlanner {
    fn default() -> Self {
        Self::new()
    }
}

impl Planner for WitPlanner {
    fn plan(&self, objective: &str) -> Result<Option<Vec<String>>, PlannerError> {
        // Call the WIT interface
        // Note: The WIT definition returns a Plan struct with Subtasks.
        // We map these to simple strings for this MVP step.

        match wit_bindings::brio::core::planner::decompose(objective) {
            Ok(plan) => {
                if plan.steps.is_empty() {
                    Ok(None)
                } else {
                    Ok(Some(
                        plan.steps.into_iter().map(|s| s.description).collect(),
                    ))
                }
            }
            Err(e) => Err(PlannerError(e)),
        }
    }
}
