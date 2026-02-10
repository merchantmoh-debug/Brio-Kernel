//! REST API endpoints for session management.
//!
//! This module provides HTTP endpoints for managing VFS sessions in the Brio system.
//! Sessions provide isolated file system operations with copy-on-write semantics.

pub mod handlers;
pub mod routes;
pub mod types;

pub use handlers::ApiError;
pub use routes::routes;
pub use types::{
    CreateSessionRequest, HealthResponse, ListSessionsResponse, SessionCommitResponse,
    SessionResponse,
};

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn test_create_session_request_deserialization() {
        let json = r#"{"base_path": "./src"}"#;
        let req: types::CreateSessionRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.base_path, "./src");
    }

    #[test]
    fn test_session_response_serialization() {
        let response = types::SessionResponse {
            id: "test-id".to_string(),
            base_path: "./src".to_string(),
            created_at: Utc::now(),
            status: "active".to_string(),
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"id\":\"test-id\""));
        assert!(json.contains("\"status\":\"active\""));
    }

    #[test]
    fn test_list_sessions_response_serialization() {
        let response = types::ListSessionsResponse {
            sessions: vec![types::SessionResponse {
                id: "test-id".to_string(),
                base_path: "./src".to_string(),
                created_at: Utc::now(),
                status: "active".to_string(),
            }],
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"sessions\""));
        assert!(json.contains("\"id\":\"test-id\""));
    }

    #[test]
    fn test_health_response_serialization() {
        let response = types::HealthResponse {
            status: "healthy".to_string(),
            timestamp: Utc::now(),
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"status\":\"healthy\""));
    }

    #[test]
    fn test_session_commit_response_serialization() {
        let response = types::SessionCommitResponse {
            session_id: "test-id".to_string(),
            status: "committed".to_string(),
            committed_at: Utc::now(),
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"session_id\":\"test-id\""));
        assert!(json.contains("\"status\":\"committed\""));
    }
}
