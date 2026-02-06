//! Supervisor Wasm Component
//!
//! The Supervisor is the "brain" of Brio. It:
//! 1. Queries the `tasks` table for pending work
//! 2. Dispatches tasks to Agents via the Service Mesh
//! 3. Updates task status based on results
//!
//! This component is built as a WASI Preview 2 module.

#![deny(missing_docs)]
#![warn(clippy::pedantic)]

pub mod domain;
pub mod handlers;
pub mod mesh_client;
pub mod orchestrator;
pub mod planner;
pub mod repository;
pub mod selector;
pub mod wit_bindings;

/// WIT bindings for the brio-host world.
///
/// This module contains auto-generated bindings from the WIT interface definitions.
/// These bindings provide the low-level interface between the supervisor and the Brio runtime.
#[cfg(target_arch = "wasm32")]
#[allow(missing_docs)]
pub mod wit_bindings {
    wit_bindgen::generate!({
        world: "brio-host",
        path: "../../wit",
    });
}

#[cfg(target_arch = "wasm32")]
pub use wit_bindings::*;

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
///
/// # Safety
/// The `no_mangle` attribute is required for FFI compatibility with the WASM runtime.
/// This function uses C calling convention for host-guest interoperability.
/// It is safe because:
/// - The function signature is fixed and known to the host
/// - No Rust-specific types cross the FFI boundary (returns i32)
/// - The function is stateless and reentrant
#[unsafe(no_mangle)]
pub extern "C" fn run() -> i32 {
    match run_inner() {
        // Use i64 as intermediate to avoid overflow, then clamp to i32 range.
        // unwrap_or is safe here: if count exceeds i32::MAX, we saturate at MAX
        // rather than panicking, ensuring the host receives a valid return value.
        Ok(count) => i64::from(count).try_into().unwrap_or(i32::MAX),
        Err(_) => -1,
    }
}

/// Inner implementation for testability.
fn run_inner() -> Result<u32, orchestrator::SupervisorError> {
    use selector::KeywordAgentSelector;
    let repository = WitTaskRepository::new();
    let dispatcher = WitAgentDispatcher::new();
    let planner = WitPlanner::new();
    let selector = KeywordAgentSelector;
    let supervisor = Supervisor::new(repository, dispatcher, planner, selector);

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
