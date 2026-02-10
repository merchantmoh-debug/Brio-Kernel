//! API Handler implementations for branch management.
//!
//! This module provides HTTP request handlers for branch operations.

use axum::{
    extract::{Json, Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde_json::json;
use std::sync::Arc;

use crate::api::branches::types::{
    BranchNodeResponse, BranchResponse, BranchTreeResponse, CreateBranchRequest,
    ExecuteBranchRequest, ListBranchesQuery, MergeRequest, MergeResponse, branch_to_response,
};
use crate::branch_manager::{
    AgentAssignment, BranchError, BranchId, BranchManager, ExecutionStrategy, MergeRequestId,
};
use crate::host::BrioHostState;

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
            ApiError::Branch(BranchError::MaxBranchesExceeded { current, limit }) => (
                StatusCode::CONFLICT,
                format!("Max branches exceeded: {current}/{limit}"),
            ),
            ApiError::Branch(BranchError::BranchAlreadyExists(name)) => (
                StatusCode::CONFLICT,
                format!("Branch already exists: {name}"),
            ),
            ApiError::Branch(BranchError::InvalidStateTransition { from, to }) => (
                StatusCode::CONFLICT,
                format!("Invalid state transition from {from} to {to}"),
            ),
            ApiError::Branch(BranchError::ExecutionFailed(msg)) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Execution failed: {msg}"),
            ),
            ApiError::Branch(BranchError::MergeConflict { file_path, .. }) => (
                StatusCode::CONFLICT,
                format!("Merge conflict in file: {file_path}"),
            ),
            ApiError::InvalidBranchId(id) => {
                (StatusCode::BAD_REQUEST, format!("Invalid branch ID: {id}"))
            }
            ApiError::InvalidMergeRequestId(id) => (
                StatusCode::BAD_REQUEST,
                format!("Invalid merge request ID: {id}"),
            ),
            ApiError::MergeNotFound(id) => (
                StatusCode::NOT_FOUND,
                format!("Merge request not found: {id}"),
            ),
            ApiError::ValidationError(msg) => (StatusCode::BAD_REQUEST, msg.clone()),
            ApiError::BranchManagerNotInitialized => (
                StatusCode::SERVICE_UNAVAILABLE,
                "Branch manager not initialized".to_string(),
            ),
            ApiError::Branch(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Internal server error".to_string(),
            ),
        };

        let body = Json(json!({
            "error": message,
            "error_type": format!("{:?}", std::mem::discriminant(&self))
        }));

        (status, body).into_response()
    }
}

/// POST /api/v1/branches
///
/// Create a new branch.
///
/// # Errors
///
/// Returns an error if:
/// - The branch manager is not initialized
/// - The branch ID is invalid
/// - The branch operation fails
pub async fn create_branch(
    State(state): State<Arc<BrioHostState>>,
    Json(req): Json<CreateBranchRequest>,
) -> Result<Json<BranchResponse>, ApiError> {
    let manager = get_branch_manager(&state);

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
        crate::api::branches::types::ExecutionStrategyRequest::Sequential => {
            ExecutionStrategy::Sequential
        }
        crate::api::branches::types::ExecutionStrategyRequest::Parallel { max_concurrent } => {
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
        .map_err(ApiError::Branch)?;

    Ok(Json(branch_to_response(&branch)))
}

/// GET /api/v1/branches
///
/// List all branches with optional filters.
///
/// # Errors
///
/// Returns an error if:
/// - The branch manager is not initialized
/// - The parent branch ID is invalid
/// - The list operation fails
pub async fn list_branches(
    State(state): State<Arc<BrioHostState>>,
    Query(query): Query<ListBranchesQuery>,
) -> Result<Json<Vec<BranchResponse>>, ApiError> {
    let manager = get_branch_manager(&state);

    let parent_id = match query.parent_id {
        Some(id) => Some(BranchId::new(id.clone()).map_err(|_| ApiError::InvalidBranchId(id))?),
        None => None,
    };

    let branches = manager
        .list_branches(query.status.as_deref(), parent_id.as_ref())
        .map_err(ApiError::Branch)?;

    Ok(Json(
        branches
            .into_iter()
            .map(|b| branch_to_response(&b))
            .collect(),
    ))
}

/// GET /api/v1/branches/{id}
///
/// Get a specific branch by ID.
///
/// # Errors
///
/// Returns an error if:
/// - The branch manager is not initialized
/// - The branch ID is invalid
/// - The branch is not found
pub async fn get_branch(
    State(state): State<Arc<BrioHostState>>,
    Path(id): Path<String>,
) -> Result<Json<BranchResponse>, ApiError> {
    let manager = get_branch_manager(&state);
    let branch_id = BranchId::new(id.clone()).map_err(|_| ApiError::InvalidBranchId(id))?;

    let branch = manager.get_branch(&branch_id).map_err(ApiError::Branch)?;

    Ok(Json(branch_to_response(&branch)))
}

/// DELETE /api/v1/branches/{id}
///
/// Delete a branch.
///
/// # Errors
///
/// Returns an error if:
/// - The branch manager is not initialized
/// - The branch ID is invalid
/// - The delete operation fails
pub async fn delete_branch(
    State(state): State<Arc<BrioHostState>>,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let manager = get_branch_manager(&state);
    let branch_id = BranchId::new(id.clone()).map_err(|_| ApiError::InvalidBranchId(id))?;

    manager
        .delete_branch(&branch_id)
        .map_err(ApiError::Branch)?;

    Ok(StatusCode::NO_CONTENT)
}

/// POST /api/v1/branches/{id}/execute
///
/// Execute a branch.
///
/// # Errors
///
/// Returns an error if:
/// - The branch manager is not initialized
/// - The branch ID is invalid
/// - The execution operation fails
pub async fn execute_branch(
    State(state): State<Arc<BrioHostState>>,
    Path(id): Path<String>,
    Json(req): Json<ExecuteBranchRequest>,
) -> Result<Json<BranchResponse>, ApiError> {
    let manager = get_branch_manager(&state);
    let branch_id = BranchId::new(id.clone()).map_err(|_| ApiError::InvalidBranchId(id))?;

    let branch = manager
        .execute_branch(&branch_id, Some(req.agents), req.task_description)
        .map_err(ApiError::Branch)?;

    Ok(Json(branch_to_response(&branch)))
}

/// POST /api/v1/branches/{id}/merge
///
/// Request a merge for a branch.
///
/// # Errors
///
/// Returns an error if:
/// - The branch manager is not initialized
/// - The branch ID is invalid
/// - The merge request operation fails
pub async fn request_merge(
    State(state): State<Arc<BrioHostState>>,
    Path(id): Path<String>,
    Json(req): Json<MergeRequest>,
) -> Result<Json<MergeResponse>, ApiError> {
    let manager = get_branch_manager(&state);
    let branch_id = BranchId::new(id.clone()).map_err(|_| ApiError::InvalidBranchId(id))?;

    let merge_request = manager
        .request_merge(&branch_id, req.strategy, req.requires_approval)
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
///
/// # Errors
///
/// Returns an error if:
/// - The branch manager is not initialized
/// - The branch ID is invalid
/// - The branch tree retrieval fails
pub async fn get_branch_tree(
    State(state): State<Arc<BrioHostState>>,
    Path(id): Path<String>,
) -> Result<Json<crate::api::branches::types::BranchTreeResponse>, ApiError> {
    let manager = get_branch_manager(&state);
    let branch_id = BranchId::new(id.clone()).map_err(|_| ApiError::InvalidBranchId(id))?;

    let branch = manager
        .get_branch_tree(&branch_id)
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
///
/// # Errors
///
/// Returns an error if:
/// - The branch manager is not initialized
/// - The branch ID is invalid
/// - The abort operation fails
pub async fn abort_branch(
    State(state): State<Arc<BrioHostState>>,
    Path(id): Path<String>,
) -> Result<Json<BranchResponse>, ApiError> {
    let manager = get_branch_manager(&state);
    let branch_id = BranchId::new(id.clone()).map_err(|_| ApiError::InvalidBranchId(id))?;

    let branch = manager.abort_branch(&branch_id).map_err(ApiError::Branch)?;

    Ok(Json(branch_to_response(&branch)))
}

/// POST /api/v1/merge-requests/{id}/approve
///
/// Approve a merge request.
///
/// # Errors
///
/// Returns an error if:
/// - The branch manager is not initialized
/// - The merge request ID is invalid
/// - The approve operation fails
pub async fn approve_merge(
    State(state): State<Arc<BrioHostState>>,
    Path(id): Path<String>,
) -> Result<Json<MergeResponse>, ApiError> {
    let manager = get_branch_manager(&state);
    let merge_request_id =
        MergeRequestId::new(id.clone()).map_err(|_| ApiError::InvalidMergeRequestId(id))?;

    let merge_request = manager
        .approve_merge(&merge_request_id, "system".to_string())
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
///
/// # Errors
///
/// Returns an error if:
/// - The branch manager is not initialized
/// - The merge request ID is invalid
/// - The reject operation fails
pub async fn reject_merge(
    State(state): State<Arc<BrioHostState>>,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let manager = get_branch_manager(&state);
    let merge_request_id =
        MergeRequestId::new(id.clone()).map_err(|_| ApiError::InvalidMergeRequestId(id))?;

    manager
        .reject_merge(&merge_request_id)
        .map_err(ApiError::Branch)?;

    Ok(StatusCode::NO_CONTENT)
}

/// Get the branch manager from state.
fn get_branch_manager(state: &Arc<BrioHostState>) -> Arc<BranchManager> {
    state.branch_manager()
}
