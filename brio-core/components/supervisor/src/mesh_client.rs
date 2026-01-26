//! Mesh Client Layer - Agent Dispatch
//!
//! Abstracts agent communication via the WIT `service-mesh` interface.
//! Follows Dependency Inversion: code depends on `AgentDispatcher` trait.

use crate::domain::{AgentId, Task};
use crate::wit_bindings;

// =============================================================================
// Error Types
// =============================================================================

/// Errors that can occur during mesh operations.
#[derive(Debug)]
pub enum MeshError {
    /// Target agent not found in mesh.
    AgentNotFound(String),
    /// Serialization/deserialization failed.
    SerializationError(String),
    /// Agent returned an error response.
    AgentError(String),
    /// Communication failure.
    TransportError(String),
}

impl core::fmt::Display for MeshError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::AgentNotFound(id) => write!(f, "Agent not found: {id}"),
            Self::SerializationError(msg) => write!(f, "Serialization error: {msg}"),
            Self::AgentError(msg) => write!(f, "Agent error: {msg}"),
            Self::TransportError(msg) => write!(f, "Transport error: {msg}"),
        }
    }
}

impl std::error::Error for MeshError {}

// =============================================================================
// Dispatch Result (CQS: Query returns data)
// =============================================================================

/// Result of dispatching a task to an agent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DispatchResult {
    /// Agent accepted the task.
    Accepted,
    /// Agent is currently busy.
    AgentBusy,
    /// Agent completed the task synchronously.
    Completed(String),
}

// =============================================================================
// Dispatcher Trait (Dependency Inversion)
// =============================================================================

/// Contract for agent dispatch operations.
///
/// This trait abstracts the mesh layer, enabling:
/// - Unit testing with mock implementations
/// - Swapping transport mechanisms without changing business logic
pub trait AgentDispatcher {
    /// Dispatches a task to the specified agent.
    ///
    /// # Errors
    /// Returns `MeshError` if dispatch fails.
    fn dispatch(&self, agent: &AgentId, task: &Task) -> Result<DispatchResult, MeshError>;
}

// =============================================================================
// Request/Response DTOs
// =============================================================================

/// Payload sent to an agent for task execution.
#[derive(Debug)]
struct TaskDispatchRequest {
    task_id: u64,
    content: String,
    priority: u8,
}

impl TaskDispatchRequest {
    fn to_json(&self) -> Result<String, MeshError> {
        // Manual JSON construction to avoid serde dependency in core
        Ok(format!(
            r#"{{"task_id":{},"content":"{}","priority":{}}}"#,
            self.task_id,
            self.content.replace('\\', "\\\\").replace('"', "\\\""),
            self.priority
        ))
    }
}

// =============================================================================
// WIT Implementation
// =============================================================================

/// Dispatcher implementation using WIT `service-mesh` bindings.
pub struct WitAgentDispatcher;

impl WitAgentDispatcher {
    /// Creates a new WIT-backed dispatcher.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for WitAgentDispatcher {
    fn default() -> Self {
        Self::new()
    }
}

impl AgentDispatcher for WitAgentDispatcher {
    fn dispatch(&self, agent: &AgentId, task: &Task) -> Result<DispatchResult, MeshError> {
        // Construct standard TaskContext payload
        let payload_json = format!(
            r#"{{"task_id":"{}","description":"{}","input_files":[]}}"#,
            task.id().to_string(),
            task.content().replace('\\', "\\\\").replace('"', "\\\"")
        );
        let payload = wit_bindings::service_mesh::Payload::Json(payload_json);

        // Call the "run" method on the agent (routed by Kernel)
        let response = wit_bindings::service_mesh::call(agent.as_str(), "run", payload)
            .map_err(MeshError::TransportError)?;

        match response {
            wit_bindings::service_mesh::Payload::Json(result) => {
                // Determine result. Since "run" returns a summary string on success, we assume completion.
                // In future, if async, we might get "Accepted" message.
                // For Phase 3, we assume synchronous completion for simplicy.
                Ok(DispatchResult::Completed(result))
            }
            wit_bindings::service_mesh::Payload::Binary(_) => Err(MeshError::SerializationError(
                "Unexpected binary response".to_string(),
            )),
        }
    }
}
