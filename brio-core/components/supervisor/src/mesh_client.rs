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
struct TaskContextDto {
    task_id: String,
    description: String,
    input_files: Vec<String>,
}

impl TaskContextDto {
    fn to_json(&self) -> Result<String, MeshError> {
        // formatting manually to avoid serde dependency
        let files_json = self
            .input_files
            .iter()
            .map(|f| format!("\"{}\"", f.replace('\\', "\\\\").replace('"', "\\\"")))
            .collect::<Vec<_>>()
            .join(",");

        Ok(format!(
            r#"{{"task-id":"{}","description":"{}","input-files":[{}]}}"#,
            self.task_id,
            self.description.replace('\\', "\\\\").replace('"', "\\\""),
            files_json
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
        let context = TaskContextDto {
            task_id: task.id().to_string(),
            description: task.content().to_string(),
            input_files: vec![],
        };

        let payload_json = context.to_json()?;
        let payload = wit_bindings::service_mesh::Payload::Json(payload_json);

        let response = wit_bindings::service_mesh::call(agent.as_str(), "run", payload)
            .map_err(MeshError::TransportError)?;

        match response {
            wit_bindings::service_mesh::Payload::Json(result) => {
                Ok(DispatchResult::Completed(result))
            }
            wit_bindings::service_mesh::Payload::Binary(_) => Err(MeshError::SerializationError(
                "Unexpected binary response".to_string(),
            )),
        }
    }
}
