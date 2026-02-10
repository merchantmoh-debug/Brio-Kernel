//! API Handler implementations for session management.
//!
//! This module provides HTTP request handlers for session operations.

use axum::{
    extract::{Json, Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use chrono::Utc;
use serde_json::json;
use std::sync::Arc;

use crate::api::sessions::types::{
    CreateSessionRequest, HealthResponse, ListSessionsResponse, SessionCommitResponse,
    SessionResponse, session_to_response,
};
use crate::host::BrioHostState;
use crate::vfs::SessionError;

/// API errors for session operations.
#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    /// Domain-level session error.
    #[error("Session error: {0}")]
    Session(#[from] SessionError),
    /// Invalid session ID format.
    #[error("Invalid session ID: {0}")]
    InvalidSessionId(String),
    /// Session manager not initialized.
    #[error("Session manager not initialized")]
    SessionManagerNotInitialized,
    /// Validation error.
    #[error("Validation error: {0}")]
    ValidationError(String),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            ApiError::Session(SessionError::SessionNotFound(id)) => {
                (StatusCode::NOT_FOUND, format!("Session not found: {id}"))
            }
            ApiError::Session(SessionError::BasePathNotFound(path)) => (
                StatusCode::BAD_REQUEST,
                format!("Base path not found: {path}"),
            ),
            ApiError::Session(SessionError::InvalidBasePath { path, source }) => (
                StatusCode::BAD_REQUEST,
                format!("Invalid base path '{path}': {source}"),
            ),
            ApiError::Session(SessionError::PolicyViolation(msg)) => {
                (StatusCode::FORBIDDEN, format!("Policy violation: {msg}"))
            }
            ApiError::Session(SessionError::Conflict { path, .. }) => (
                StatusCode::CONFLICT,
                format!("Conflict detected at path: {}", path.display()),
            ),
            ApiError::Session(SessionError::SessionDirectoryLost(path)) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Session directory lost: {}", path.display()),
            ),
            ApiError::Session(SessionError::CopyFailed(msg)) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to copy base directory: {msg}"),
            ),
            ApiError::Session(SessionError::DiffFailed(msg)) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Diff operation failed: {msg}"),
            ),
            ApiError::Session(SessionError::CleanupFailed { path, source }) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Cleanup failed for '{}': {source}", path.display()),
            ),
            ApiError::Session(SessionError::ReadDirectoryFailed(msg)) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to read directory: {msg}"),
            ),
            ApiError::InvalidSessionId(id) => {
                (StatusCode::BAD_REQUEST, format!("Invalid session ID: {id}"))
            }
            ApiError::SessionManagerNotInitialized => (
                StatusCode::SERVICE_UNAVAILABLE,
                "Session manager not initialized".to_string(),
            ),
            ApiError::ValidationError(msg) => (StatusCode::BAD_REQUEST, msg.clone()),
        };

        let body = Json(json!({
            "error": message,
            "error_type": format!("{:?}", std::mem::discriminant(&self))
        }));

        (status, body).into_response()
    }
}

/// GET /health
///
/// Returns basic health status of the kernel.
pub async fn health_check() -> Result<Json<HealthResponse>, ApiError> {
    Ok(Json(HealthResponse {
        status: "healthy".to_string(),
        timestamp: Utc::now(),
    }))
}

/// GET /api/v1/sessions
///
/// List all active sessions with metadata.
pub async fn list_sessions(
    State(state): State<Arc<BrioHostState>>,
) -> Result<Json<ListSessionsResponse>, ApiError> {
    let manager = state.session_manager();
    let sessions = manager.lock().list_sessions();

    let responses: Vec<SessionResponse> = sessions
        .into_iter()
        .map(|(id, info)| session_to_response(&id, info.base_path()))
        .collect();

    Ok(Json(ListSessionsResponse {
        sessions: responses,
    }))
}

/// POST /api/v1/sessions
///
/// Create a new VFS session.
pub async fn create_session(
    State(state): State<Arc<BrioHostState>>,
    Json(req): Json<CreateSessionRequest>,
) -> Result<Json<SessionResponse>, ApiError> {
    // Validate the request
    if req.base_path.is_empty() {
        return Err(ApiError::ValidationError(
            "base_path is required".to_string(),
        ));
    }

    // Create the session
    let session_id = state
        .begin_session(&req.base_path)
        .map_err(ApiError::Session)?;

    // Get the session info to return the base path
    let manager = state.session_manager();
    let _base_path = manager
        .lock()
        .session_path(&session_id)
        .unwrap_or_else(|| std::path::PathBuf::from(&req.base_path));
    // _base_path is used for future reference if needed

    Ok(Json(SessionResponse {
        id: session_id,
        base_path: req.base_path,
        created_at: Utc::now(),
        status: "active".to_string(),
    }))
}

/// DELETE /api/v1/sessions/{id}
///
/// Delete/cleanup a session by rolling it back.
pub async fn delete_session(
    State(state): State<Arc<BrioHostState>>,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    // Validate session ID (basic UUID format check)
    if id.len() < 32 || id.contains('/') {
        return Err(ApiError::InvalidSessionId(id));
    }

    // Rollback the session (cleanup)
    state.rollback_session(&id).map_err(ApiError::Session)?;

    Ok(StatusCode::NO_CONTENT)
}

/// POST /api/v1/sessions/{id}/commit
///
/// Commit a session's changes back to the base directory.
pub async fn commit_session(
    State(state): State<Arc<BrioHostState>>,
    Path(id): Path<String>,
) -> Result<Json<SessionCommitResponse>, ApiError> {
    // Validate session ID (basic UUID format check)
    if id.len() < 32 || id.contains('/') {
        return Err(ApiError::InvalidSessionId(id));
    }

    // Commit the session
    state.commit_session(&id).map_err(ApiError::Session)?;

    Ok(Json(SessionCommitResponse {
        session_id: id,
        status: "committed".to_string(),
        committed_at: Utc::now(),
    }))
}
