//! REST API routes for session management.
//!
//! This module provides HTTP endpoints for creating, managing, and deleting
//! VFS sessions in the Brio system.

use axum::{
    Router,
    routing::{delete, get, post},
};
use std::sync::Arc;

use crate::api::sessions::handlers::{
    commit_session, create_session, delete_session, health_check, list_sessions,
};
use crate::host::BrioHostState;

/// API routes for session management.
///
/// Creates a router with all session-related endpoints mounted at `/api/v1`.
/// Also includes the basic health check at `/health`.
pub fn routes() -> Router<Arc<BrioHostState>> {
    Router::new()
        .route("/health", get(health_check))
        .route("/api/v1/sessions", get(list_sessions).post(create_session))
        .route("/api/v1/sessions/{id}", delete(delete_session))
        .route("/api/v1/sessions/{id}/commit", post(commit_session))
}
