//! REST API endpoints for branch management.
//!
//! This module provides HTTP endpoints for creating, managing, and orchestrating
//! branches in the Brio system. Branches allow parallel execution of agent tasks
//! with isolated state and eventual merge capabilities.

use axum::{
    extract::{Json, Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;

use crate::host::BrioHostState;
use crate::branch_manager::{
    AgentAssignment, Branch, BranchConfig, BranchError, BranchId, BranchManager,
    BranchStatus, ExecutionStrategy, MergeRequestId, MergeRequestModel, MergeRequestStatus,
};

/// API routes for branch management.
///
/// Creates a router with all branch-related endpoints mounted at `/api/v1`.
pub fn routes() -> Router<Arc<BrioHostState>> {
    Router::new()
        .route("/api/v1/branches", post(create_branch).get(list_branches))
        .route(
            "/api/v1/branches/{id}",
            get(get_branch).delete(delete_branch),
        )
        .route("/api/v1/branches/{id}/execute", post(execute_branch))
        .route("/api/v1/branches/{id}/merge", post(request_merge))
        .route("/api/v1/branches/{id}/tree", get(get_branch_tree))
        .route("/api/v1/branches/{id}/abort", post(abort_branch))
        .route("/api/v1/merge-requests/{id}/approve", post(approve_merge))
        .route("/api/v1/merge-requests/{id}/reject", post(reject_merge))
}

// =============================================================================
// Request/Response Types
// =============================================================================

/// Request to create a new branch.
#[derive(Debug, Clone, Deserialize)]
pub struct CreateBranchRequest {
    /// Source of the branch (base directory or parent branch).
    pub source: BranchSourceRequest,
    /// Branch configuration.
    pub config: BranchConfigRequest,
}

/// Source specification for branch creation.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type")]
pub enum BranchSourceRequest {
    /// Create from a base filesystem path.
    #[serde(rename = "base")]
    Base {
        /// Path to the base directory.
        path: String,
    },
    /// Create from an existing branch.
    #[serde(rename = "branch")]
    Branch {
        /// ID of the parent branch.
        branch_id: String,
    },
}

/// Branch configuration request.
#[derive(Debug, Clone, Deserialize)]
pub struct BranchConfigRequest {
    /// Human-readable branch name.
    pub name: String,
    /// Agent assignments for this branch.
    pub agents: Vec<AgentAssignmentRequest>,
    /// Execution strategy (sequential or parallel).
    #[serde(default)]
    pub execution_strategy: ExecutionStrategyRequest,
    /// Whether to auto-merge on completion.
    #[serde(default)]
    pub auto_merge: bool,
    /// Merge strategy to use.
    #[serde(default = "default_merge_strategy")]
    pub merge_strategy: String,
}

fn default_merge_strategy() -> String {
    "union".to_string()
}

/// Execution strategy for agent tasks within a branch.
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(tag = "type")]
pub enum ExecutionStrategyRequest {
    /// Execute agents sequentially in order.
    #[default]
    #[serde(rename = "sequential")]
    Sequential,
    /// Execute agents in parallel with optional concurrency limit.
    #[serde(rename = "parallel")]
    Parallel {
        /// Maximum number of concurrent agents that can run simultaneously.
        /// If None, uses system default based on available resources.
        max_concurrent: Option<usize>,
    },
}

/// Agent assignment within a branch.
#[derive(Debug, Clone, Deserialize)]
pub struct AgentAssignmentRequest {
    /// ID of the agent to assign.
    pub agent_id: String,
    /// Optional task description override.
    pub task_override: Option<String>,
    /// Priority level (higher = executed first in sequential mode).
    #[serde(default)]
    pub priority: u8,
}

/// Branch response payload.
#[derive(Debug, Clone, Serialize)]
pub struct BranchResponse {
    /// Branch unique identifier.
    pub id: String,
    /// Parent branch ID (None for root branches).
    pub parent_id: Option<String>,
    /// Branch name.
    pub name: String,
    /// Current status (pending, running, completed, failed, aborted).
    pub status: String,
    /// Session ID for VFS isolation.
    pub session_id: String,
    /// Creation timestamp (ISO8601).
    pub created_at: String,
    /// Completion timestamp (ISO8601), None if not completed.
    pub completed_at: Option<String>,
    /// Child branch IDs.
    pub children: Vec<String>,
}

/// Execute branch request.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct ExecuteBranchRequest {
    /// Specific agents to execute (empty = all assigned agents).
    #[serde(default)]
    pub agents: Vec<String>,
    /// Optional task description override.
    #[serde(default)]
    pub task_description: Option<String>,
}

/// Merge request payload.
#[derive(Debug, Clone, Deserialize)]
pub struct MergeRequest {
    /// Merge strategy to apply.
    #[serde(default = "default_merge_strategy")]
    pub strategy: String,
    /// Whether approval is required before merging.
    #[serde(default)]
    pub requires_approval: bool,
}

/// Merge response payload.
#[derive(Debug, Clone, Serialize)]
pub struct MergeResponse {
    /// Merge request unique identifier.
    pub merge_request_id: String,
    /// Current status (pending, approved, rejected, merged, conflict).
    pub status: String,
    /// Whether approval is still required.
    pub requires_approval: bool,
    /// Conflicts if any detected.
    pub conflicts: Option<Vec<ConflictResponse>>,
}

/// Conflict information for merge operations.
#[derive(Debug, Clone, Serialize)]
pub struct ConflictResponse {
    /// File path where conflict occurred.
    pub file_path: String,
    /// Type of conflict.
    pub conflict_type: String,
    /// Branch IDs involved in the conflict.
    pub branches: Vec<String>,
}

/// Branch tree response.
#[derive(Debug, Clone, Serialize)]
pub struct BranchTreeResponse {
    /// Root node of the tree.
    pub root: BranchNodeResponse,
}

/// Node in the branch tree hierarchy.
#[derive(Debug, Clone, Serialize)]
pub struct BranchNodeResponse {
    /// Branch ID.
    pub id: String,
    /// Branch name.
    pub name: String,
    /// Branch status.
    pub status: String,
    /// Child nodes.
    pub children: Vec<BranchNodeResponse>,
}

/// Query parameters for listing branches.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct ListBranchesQuery {
    /// Filter by status.
    pub status: Option<String>,
    /// Filter by parent branch ID.
    pub parent_id: Option<String>,
}

/// Request to reject a merge.
#[derive(Debug, Clone, Deserialize)]
pub struct RejectMergeRequest {
    /// Reason for rejection (optional).
    pub reason: Option<String>,
}

// =============================================================================
// Error Handling
// =============================================================================

/// API errors for branch operations.
#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    /// Domain-level branch error.
    #[error("Branch error: {0}")]
    Branch(#[from] BranchError),
    /// Invalid branch ID format.
    #[error("Invalid branch ID: {0}")]
    InvalidBranchId(String),
    /// Invalid merge request ID format.
    #[error("Invalid merge request ID: {0}")]
    InvalidMergeRequestId(String),
    /// Merge request not found.
    #[error("Merge request not found: {0}")]
    MergeNotFound(String),
    /// Validation error.
    #[error("Validation error: {0}")]
    ValidationError(String),
    /// Branch manager not initialized.
    #[error("Branch manager not initialized")]
    BranchManagerNotInitialized,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            ApiError::Branch(BranchError::BranchNotFound(id)) => {
                (StatusCode::NOT_FOUND, format!("Branch not found: {id}"))
            }
            ApiError::Branch(BranchError::MaxBranchesExceeded { current, limit }) => {
                (
                    StatusCode::CONFLICT,
                    format!("Max branches exceeded: {current}/{limit}"),
                )
            }
            ApiError::Branch(BranchError::BranchAlreadyExists(name)) => {
                (StatusCode::CONFLICT, format!("Branch already exists: {name}"))
            }
            ApiError::Branch(BranchError::InvalidStateTransition { from, to }) => {
                (
                    StatusCode::CONFLICT,
                    format!("Invalid state transition from {from} to {to}"),
                )
            }
            ApiError::Branch(BranchError::ExecutionFailed(msg)) => {
                (StatusCode::INTERNAL_SERVER_ERROR, format!("Execution failed: {msg}"))
            }
            ApiError::Branch(BranchError::MergeConflict { file_path, .. }) => {
                (
                    StatusCode::CONFLICT,
                    format!("Merge conflict in file: {file_path}"),
                )
            }
            ApiError::InvalidBranchId(id) => {
                (StatusCode::BAD_REQUEST, format!("Invalid branch ID: {id}"))
            }
            ApiError::InvalidMergeRequestId(id) => {
                (StatusCode::BAD_REQUEST, format!("Invalid merge request ID: {id}"))
            }
            ApiError::MergeNotFound(id) => {
                (StatusCode::NOT_FOUND, format!("Merge request not found: {id}"))
            }
            ApiError::ValidationError(msg) => (StatusCode::BAD_REQUEST, msg.clone()),
            ApiError::BranchManagerNotInitialized => (
                StatusCode::SERVICE_UNAVAILABLE,
                "Branch manager not initialized".to_string(),
            ),
            _ => (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error".to_string()),
        };

        let body = Json(json!({
            "error": message,
            "error_type": format!("{:?}", std::mem::discriminant(&self))
        }));

        (status, body).into_response()
    }
}

// =============================================================================
// API Handlers
// =============================================================================

/// POST /api/v1/branches
///
/// Create a new branch.
async fn create_branch(
    State(state): State<Arc<BrioHostState>>,
    Json(req): Json<CreateBranchRequest>,
) -> Result<Json<BranchResponse>, ApiError> {
    let manager = get_branch_manager(&state).ok_or(ApiError::BranchManagerNotInitialized)?;
    
    let agents: Vec<AgentAssignment> = req
        .config
        .agents
        .into_iter()
        .map(|a| AgentAssignment {
            agent_id: a.agent_id,
            task_override: a.task_override,
            priority: a.priority,
        })
        .collect();

    let execution_strategy = match req.config.execution_strategy {
        ExecutionStrategyRequest::Sequential => ExecutionStrategy::Sequential,
        ExecutionStrategyRequest::Parallel { max_concurrent } => {
            ExecutionStrategy::Parallel { max_concurrent }
        }
    };

    let branch = manager
        .create_branch(
            req.config.name,
            agents,
            execution_strategy,
            req.config.auto_merge,
            req.config.merge_strategy,
        )
        .await
        .map_err(ApiError::Branch)?;

    Ok(Json(branch_to_response(&branch)))
}

/// GET /api/v1/branches
///
/// List all branches with optional filters.
async fn list_branches(
    State(state): State<Arc<BrioHostState>>,
    Query(query): Query<ListBranchesQuery>,
) -> Result<Json<Vec<BranchResponse>>, ApiError> {
    let manager = get_branch_manager(&state).ok_or(ApiError::BranchManagerNotInitialized)?;

    let parent_id = match query.parent_id {
        Some(id) => Some(BranchId::new(id.clone()).map_err(|_| ApiError::InvalidBranchId(id))?),
        None => None,
    };

    let branches = manager
        .list_branches(query.status.as_deref(), parent_id.as_ref())
        .await
        .map_err(ApiError::Branch)?;

    Ok(Json(branches.into_iter().map(|b| branch_to_response(&b)).collect()))
}

/// GET /api/v1/branches/{id}
///
/// Get a specific branch by ID.
async fn get_branch(
    State(state): State<Arc<BrioHostState>>,
    Path(id): Path<String>,
) -> Result<Json<BranchResponse>, ApiError> {
    let manager = get_branch_manager(&state).ok_or(ApiError::BranchManagerNotInitialized)?;
    let branch_id = BranchId::new(id.clone()).map_err(|_| ApiError::InvalidBranchId(id))?;

    let branch = manager
        .get_branch(&branch_id)
        .await
        .map_err(ApiError::Branch)?;

    Ok(Json(branch_to_response(&branch)))
}

/// DELETE /api/v1/branches/{id}
///
/// Delete a branch.
async fn delete_branch(
    State(state): State<Arc<BrioHostState>>,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let manager = get_branch_manager(&state).ok_or(ApiError::BranchManagerNotInitialized)?;
    let branch_id = BranchId::new(id.clone()).map_err(|_| ApiError::InvalidBranchId(id))?;

    manager
        .delete_branch(&branch_id)
        .await
        .map_err(ApiError::Branch)?;

    Ok(StatusCode::NO_CONTENT)
}

/// POST /api/v1/branches/{id}/execute
///
/// Execute a branch.
async fn execute_branch(
    State(state): State<Arc<BrioHostState>>,
    Path(id): Path<String>,
    Json(req): Json<ExecuteBranchRequest>,
) -> Result<Json<BranchResponse>, ApiError> {
    let manager = get_branch_manager(&state).ok_or(ApiError::BranchManagerNotInitialized)?;
    let branch_id = BranchId::new(id.clone()).map_err(|_| ApiError::InvalidBranchId(id))?;

    let branch = manager
        .execute_branch(&branch_id, Some(req.agents), req.task_description)
        .await
        .map_err(ApiError::Branch)?;

    Ok(Json(branch_to_response(&branch)))
}

/// POST /api/v1/branches/{id}/merge
///
/// Request a merge for a branch.
async fn request_merge(
    State(state): State<Arc<BrioHostState>>,
    Path(id): Path<String>,
    Json(req): Json<MergeRequest>,
) -> Result<Json<MergeResponse>, ApiError> {
    let manager = get_branch_manager(&state).ok_or(ApiError::BranchManagerNotInitialized)?;
    let branch_id = BranchId::new(id.clone()).map_err(|_| ApiError::InvalidBranchId(id))?;

    let merge_request = manager
        .request_merge(&branch_id, req.strategy, req.requires_approval)
        .await
        .map_err(ApiError::Branch)?;

    Ok(Json(MergeResponse {
        merge_request_id: merge_request.id.to_string(),
        status: merge_request.status.to_string(),
        requires_approval: merge_request.requires_approval,
        conflicts: None,
    }))
}

/// GET /api/v1/branches/{id}/tree
///
/// Get the branch tree.
async fn get_branch_tree(
    State(state): State<Arc<BrioHostState>>,
    Path(id): Path<String>,
) -> Result<Json<BranchTreeResponse>, ApiError> {
    let manager = get_branch_manager(&state).ok_or(ApiError::BranchManagerNotInitialized)?;
    let branch_id = BranchId::new(id.clone()).map_err(|_| ApiError::InvalidBranchId(id))?;

    let branch = manager
        .get_branch_tree(&branch_id)
        .await
        .map_err(ApiError::Branch)?;

    Ok(Json(BranchTreeResponse {
        root: BranchNodeResponse {
            id: branch.id.to_string(),
            name: branch.name,
            status: branch.status.to_string(),
            children: branch
                .children
                .iter()
                .map(|c| BranchNodeResponse {
                    id: c.to_string(),
                    name: String::new(),
                    status: String::new(),
                    children: vec![],
                })
                .collect(),
        },
    }))
}

/// POST /api/v1/branches/{id}/abort
///
/// Abort a branch.
async fn abort_branch(
    State(state): State<Arc<BrioHostState>>,
    Path(id): Path<String>,
) -> Result<Json<BranchResponse>, ApiError> {
    let manager = get_branch_manager(&state).ok_or(ApiError::BranchManagerNotInitialized)?;
    let branch_id = BranchId::new(id.clone()).map_err(|_| ApiError::InvalidBranchId(id))?;

    let branch = manager
        .abort_branch(&branch_id)
        .await
        .map_err(ApiError::Branch)?;

    Ok(Json(branch_to_response(&branch)))
}

/// POST /api/v1/merge-requests/{id}/approve
///
/// Approve a merge request.
async fn approve_merge(
    State(state): State<Arc<BrioHostState>>,
    Path(id): Path<String>,
) -> Result<Json<MergeResponse>, ApiError> {
    let manager = get_branch_manager(&state).ok_or(ApiError::BranchManagerNotInitialized)?;
    let merge_request_id = MergeRequestId::new(id.clone()).map_err(|_| ApiError::InvalidMergeRequestId(id))?;

    let merge_request = manager
        .approve_merge(&merge_request_id, "system".to_string())
        .await
        .map_err(ApiError::Branch)?;

    Ok(Json(MergeResponse {
        merge_request_id: merge_request.id.to_string(),
        status: merge_request.status.to_string(),
        requires_approval: false,
        conflicts: None,
    }))
}

/// POST /api/v1/merge-requests/{id}/reject
///
/// Reject a merge request.
async fn reject_merge(
    State(state): State<Arc<BrioHostState>>,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let manager = get_branch_manager(&state).ok_or(ApiError::BranchManagerNotInitialized)?;
    let merge_request_id = MergeRequestId::new(id.clone()).map_err(|_| ApiError::InvalidMergeRequestId(id))?;

    manager
        .reject_merge(&merge_request_id)
        .await
        .map_err(ApiError::Branch)?;

    Ok(StatusCode::NO_CONTENT)
}

// =============================================================================
// Helper Functions
// =============================================================================

/// Get the branch manager from state.
fn get_branch_manager(state: &Arc<BrioHostState>) -> Option<Arc<BranchManager>> {
    Some(state.branch_manager())
}

/// Convert a Branch domain model to API response.
fn branch_to_response(branch: &Branch) -> BranchResponse {
    BranchResponse {
        id: branch.id.as_str().to_string(),
        parent_id: branch.parent_id.as_ref().map(|p| p.as_str().to_string()),
        name: branch.name.clone(),
        status: branch.status.to_string(),
        session_id: branch.session_id.clone(),
        created_at: branch.created_at.to_rfc3339(),
        completed_at: branch.completed_at.map(|t| t.to_rfc3339()),
        children: branch.children.iter().map(|c| c.as_str().to_string()).collect(),
    }
}


// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // Test request/response serialization
    #[test]
    fn test_create_branch_request_deserialization() {
        let json = r#"{
            "source": {
                "type": "base",
                "path": "/workspace"
            },
            "config": {
                "name": "feature-branch",
                "agents": [
                    {
                        "agent_id": "agent-1",
                        "priority": 1
                    }
                ]
            }
        }"#;

        let req: CreateBranchRequest = serde_json::from_str(json).unwrap();
        assert!(matches!(req.source, BranchSourceRequest::Base { path } if path == "/workspace"));
        assert_eq!(req.config.name, "feature-branch");
        assert_eq!(req.config.agents.len(), 1);
        assert_eq!(req.config.agents[0].agent_id, "agent-1");
    }

    #[test]
    fn test_branch_source_branch_deserialization() {
        let json = r#"{
            "type": "branch",
            "branch_id": "parent-branch-uuid"
        }"#;

        let source: BranchSourceRequest = serde_json::from_str(json).unwrap();
        assert!(matches!(source, BranchSourceRequest::Branch { branch_id } if branch_id == "parent-branch-uuid"));
    }

    #[test]
    fn test_execution_strategy_deserialization() {
        // Test Parallel variant with tagged deserialization
        let json = r#"{"type": "parallel", "max_concurrent": 5}"#;
        let strategy: ExecutionStrategyRequest = serde_json::from_str(json).unwrap();
        assert!(
            matches!(strategy, ExecutionStrategyRequest::Parallel { max_concurrent } if max_concurrent == Some(5))
        );

        // Test Sequential variant explicitly
        let json = r#"{"type": "sequential"}"#;
        let strategy: ExecutionStrategyRequest = serde_json::from_str(json).unwrap();
        assert!(matches!(strategy, ExecutionStrategyRequest::Sequential));
    }

    #[test]
    fn test_branch_response_serialization() {
        let response = BranchResponse {
            id: "test-id".to_string(),
            parent_id: Some("parent-id".to_string()),
            name: "test-branch".to_string(),
            status: "pending".to_string(),
            session_id: "session-1".to_string(),
            created_at: "2024-01-01T00:00:00Z".to_string(),
            completed_at: None,
            children: vec!["child-1".to_string()],
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"id\":\"test-id\""));
        assert!(json.contains("\"status\":\"pending\""));
    }

    #[test]
    fn test_merge_response_serialization() {
        let response = MergeResponse {
            merge_request_id: "mr-1".to_string(),
            status: "pending".to_string(),
            requires_approval: true,
            conflicts: Some(vec![ConflictResponse {
                file_path: "test.txt".to_string(),
                conflict_type: "content".to_string(),
                branches: vec!["branch-1".to_string()],
            }]),
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"merge_request_id\":\"mr-1\""));
        assert!(json.contains("\"conflicts\""));
    }

    #[test]
    fn test_branch_tree_serialization() {
        let tree = BranchTreeResponse {
            root: BranchNodeResponse {
                id: "root".to_string(),
                name: "main".to_string(),
                status: "completed".to_string(),
                children: vec![BranchNodeResponse {
                    id: "child".to_string(),
                    name: "feature".to_string(),
                    status: "running".to_string(),
                    children: vec![],
                }],
            },
        };

        let json = serde_json::to_string(&tree).unwrap();
        assert!(json.contains("\"root\""));
        assert!(json.contains("\"children\""));
    }

    // Test ID validation
    #[test]
    fn test_branch_id_validation() {
        // Valid UUID-like format
        let id = BranchId::new("550e8400-e29b-41d4-a716-446655440000".to_string());
        assert!(id.is_ok());

        // Empty ID
        let id = BranchId::new("".to_string());
        assert!(matches!(id, Err(ApiError::InvalidBranchId(_))));
    }

    #[test]
    fn test_merge_request_id_validation() {
        let id = MergeRequestId::new("mr-123".to_string());
        assert!(id.is_ok());

        let id = MergeRequestId::new("".to_string());
        assert!(matches!(id, Err(ApiError::InvalidMergeRequestId(_))));
    }

    // Test default values
    #[test]
    fn test_default_execution_strategy() {
        let default = ExecutionStrategyRequest::default();
        assert!(matches!(default, ExecutionStrategyRequest::Sequential));
    }

    #[test]
    fn test_default_merge_strategy() {
        assert_eq!(default_merge_strategy(), "union");
    }

    // Test BranchManager
    #[tokio::test]
    async fn test_branch_manager_create_and_get() {
        let manager = BranchManager::new();
        
        let config = BranchConfigRequest {
            name: "test-branch".to_string(),
            agents: vec![],
            execution_strategy: ExecutionStrategyRequest::Sequential,
            auto_merge: false,
            merge_strategy: "union".to_string(),
        };

        let branch = manager
            .create_branch(config.name.clone(), vec![], ExecutionStrategy::Sequential, config.auto_merge, config.merge_strategy.clone())
            .await
            .unwrap();

        assert_eq!(branch.name, "test-branch");
        assert!(matches!(branch.status, BranchStatus::Pending));

        // Get the branch
        let retrieved = manager.get_branch(&branch.id).await.unwrap();
        assert_eq!(retrieved.name, branch.name);
    }

    #[tokio::test]
    async fn test_branch_manager_list() {
        let manager = BranchManager::new();
        
        let config = BranchConfigRequest {
            name: "branch-1".to_string(),
            agents: vec![],
            execution_strategy: ExecutionStrategyRequest::Sequential,
            auto_merge: false,
            merge_strategy: "union".to_string(),
        };

        manager
            .create_branch(config.name.clone(), vec![], ExecutionStrategy::Sequential, config.auto_merge, config.merge_strategy.clone())
            .await
            .unwrap();

        let branches = manager.list_branches(None, None).await.unwrap();
        assert_eq!(branches.len(), 1);
        assert_eq!(branches[0].name, "branch-1");
    }

    #[tokio::test]
    async fn test_branch_manager_delete() {
        let manager = BranchManager::new();
        
        let config = BranchConfigRequest {
            name: "to-delete".to_string(),
            agents: vec![],
            execution_strategy: ExecutionStrategyRequest::Sequential,
            auto_merge: false,
            merge_strategy: "union".to_string(),
        };

        let branch = manager
            .create_branch(config.name.clone(), vec![], ExecutionStrategy::Sequential, config.auto_merge, config.merge_strategy.clone())
            .await
            .unwrap();

        manager.delete_branch(&branch.id).await.unwrap();

        let result = manager.get_branch(&branch.id).await;
        assert!(matches!(result, Err(BranchError::BranchNotFound(_))));
    }

    #[tokio::test]
    async fn test_merge_request_workflow() {
        let manager = BranchManager::new();
        
        let config = BranchConfigRequest {
            name: "merge-test".to_string(),
            agents: vec![],
            execution_strategy: ExecutionStrategyRequest::Sequential,
            auto_merge: false,
            merge_strategy: "union".to_string(),
        };

        let branch = manager
            .create_branch(config.name.clone(), vec![], ExecutionStrategy::Sequential, config.auto_merge, config.merge_strategy.clone())
            .await
            .unwrap();

        let merge_req = MergeRequest {
            strategy: "union".to_string(),
            requires_approval: true,
        };

        let mr = manager.request_merge(&branch.id, &merge_req).await.unwrap();
        assert_eq!(mr.status, MergeRequestStatus::Pending);
        assert!(mr.requires_approval);

        // Approve
        let approved = manager.approve_merge(&mr.id).await.unwrap();
        assert_eq!(approved.status, MergeRequestStatus::Approved);
    }

    // Test error conversions
    #[test]
    fn test_api_error_into_response() {
        let error = ApiError::InvalidBranchId("bad-id".to_string());
        let response = error.into_response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        let error = ApiError::Branch(BranchError::BranchNotFound("id".to_string()));
        let response = error.into_response();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);

        let error = ApiError::Branch(BranchError::MaxBranchesExceeded { current: 10, limit: 10 });
        let response = error.into_response();
        assert_eq!(response.status(), StatusCode::CONFLICT);
    }

    // Test router creation
    #[test]
    fn test_routes_creates_valid_router() {
        let _router = routes();
        // Just verify it doesn't panic and has the expected routes
        // Full integration testing would require spinning up a test server
    }
}
