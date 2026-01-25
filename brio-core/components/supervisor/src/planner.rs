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
    fn plan(&self, objective: &str) -> Result<(), PlannerError> {
        // Call the WIT interface
        // Note: The current WIT definition returns a result<plan, string>.
        // For this MVP, we just trigger the decomposition and ignore the plan content
        // (assuming side effects or purely state transition focus for now).
        // In a real implementation, we would store the subtasks.

        match wit_bindings::brio::core::planner::decompose(objective) {
            Ok(_plan) => Ok(()),
            Err(e) => Err(PlannerError(e)),
        }
    }
}
