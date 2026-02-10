//! Request/Response Types for Session API
//!
//! This module provides DTOs for session management operations.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Request to create a new session.
#[derive(Debug, Clone, Deserialize)]
pub struct CreateSessionRequest {
    /// Base path for the session (directory to copy).
    pub base_path: String,
}

/// Session response payload.
#[derive(Debug, Clone, Serialize)]
pub struct SessionResponse {
    /// Session unique identifier (UUID).
    pub id: String,
    /// Base path that was copied.
    pub base_path: String,
    /// Session creation timestamp.
    pub created_at: DateTime<Utc>,
    /// Current status (active, committing, `rolled_back`).
    pub status: String,
}

/// List sessions response payload.
#[derive(Debug, Clone, Serialize)]
pub struct ListSessionsResponse {
    /// List of active sessions.
    pub sessions: Vec<SessionResponse>,
}

/// Health check response payload.
#[derive(Debug, Clone, Serialize)]
pub struct HealthResponse {
    /// Health status.
    pub status: String,
    /// Timestamp of the check.
    pub timestamp: DateTime<Utc>,
}

/// Session commit response payload.
#[derive(Debug, Clone, Serialize)]
pub struct SessionCommitResponse {
    /// Session ID that was committed.
    pub session_id: String,
    /// Commit status.
    pub status: String,
    /// Commit timestamp.
    pub committed_at: DateTime<Utc>,
}

/// Convert session info from the manager to API response.
#[must_use]
pub fn session_to_response(id: &str, base_path: &std::path::Path) -> SessionResponse {
    SessionResponse {
        id: id.to_string(),
        base_path: base_path.to_string_lossy().to_string(),
        created_at: Utc::now(),
        status: "active".to_string(),
    }
}
