//! REST API routes for branch management.
//!
//! This module provides HTTP endpoints for creating, managing, and orchestrating
//! branches in the Brio system.

use axum::{
    Router,
    routing::{get, post},
};
use std::sync::Arc;

use crate::api::branches::handlers::{
    abort_branch, approve_merge, create_branch, delete_branch, execute_branch, get_branch,
    get_branch_tree, list_branches, reject_merge, request_merge,
};
use crate::host::BrioHostState;

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
