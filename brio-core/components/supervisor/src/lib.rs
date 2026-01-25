//! Supervisor Wasm Component
//!
//! The Supervisor is the "brain" of Brio. It:
//! 1. Queries the `tasks` table for pending work
//! 2. Dispatches tasks to Agents via the Service Mesh
//! 3. Updates task status based on results
//!
//! This component is built as a WASI Preview 2 module.

pub mod domain;
pub mod mesh_client;
pub mod orchestrator;
pub mod planner;
pub mod repository;
pub mod wit_bindings;

// Generate WIT bindings when building for WASM target
#[cfg(target_arch = "wasm32")]
wit_bindgen::generate!({
    world: "brio-host",
    path: "../../wit",
});

use mesh_client::WitAgentDispatcher;
use orchestrator::Supervisor;
use planner::WitPlanner;
use repository::WitTaskRepository;

/// Guest export: Run a single supervision cycle.
///
/// Called by the Host to process pending tasks.
/// Returns the number of successfully dispatched tasks.
///
/// # Errors
/// Returns error string if the supervision cycle fails critically.
#[unsafe(no_mangle)]
pub extern "C" fn run() -> i32 {
    match run_inner() {
        Ok(count) => count as i32,
        Err(_) => -1,
    }
}

/// Inner implementation for testability.
fn run_inner() -> Result<u32, orchestrator::SupervisorError> {
    let repository = WitTaskRepository::new();
    let dispatcher = WitAgentDispatcher::new();
    let planner = WitPlanner::new();
    let supervisor = Supervisor::new(repository, dispatcher, planner);

    supervisor.poll_tasks()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_inner_compiles() {
        // Smoke test: verifies the wiring compiles
        // Actual execution requires WASM runtime
        let _ = run_inner;
    }
}
