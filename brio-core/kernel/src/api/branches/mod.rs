//! REST API endpoints for branch management.
//!
//! This module provides HTTP endpoints for creating, managing, and orchestrating
//! branches in the Brio system. Branches allow parallel execution of agent tasks
//! with isolated state and eventual merge capabilities.

pub mod handlers;
pub mod routes;
pub mod types;

pub use handlers::ApiError;
pub use routes::routes;
pub use types::{
    AgentAssignmentRequest, BranchConfigRequest, BranchNodeResponse, BranchResponse,
    BranchSourceRequest, BranchTreeResponse, ConflictResponse, CreateBranchRequest,
    ExecuteBranchRequest, ExecutionStrategyRequest, ListBranchesQuery, MergeRequest, MergeResponse,
    RejectMergeRequest,
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::branch_manager::{
        BranchError, BranchId, BranchManager, BranchStatus,
        ExecutionStrategy, MergeRequestId, MergeRequestStatus,
    };

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

        let req: types::CreateBranchRequest = serde_json::from_str(json).unwrap();
        assert!(
            matches!(req.source, types::BranchSourceRequest::Base { path } if path == "/workspace")
        );
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

        let source: types::BranchSourceRequest = serde_json::from_str(json).unwrap();
        assert!(
            matches!(source, types::BranchSourceRequest::Branch { branch_id } if branch_id == "parent-branch-uuid")
        );
    }

    #[test]
    fn test_execution_strategy_deserialization() {
        // Test Parallel variant with tagged deserialization
        let json = r#"{"type": "parallel", "max_concurrent": 5}"#;
        let strategy: types::ExecutionStrategyRequest = serde_json::from_str(json).unwrap();
        assert!(
            matches!(strategy, types::ExecutionStrategyRequest::Parallel { max_concurrent } if max_concurrent == Some(5))
        );

        // Test Sequential variant explicitly
        let json = r#"{"type": "sequential"}"#;
        let strategy: types::ExecutionStrategyRequest = serde_json::from_str(json).unwrap();
        assert!(matches!(
            strategy,
            types::ExecutionStrategyRequest::Sequential
        ));
    }

    #[test]
    fn test_branch_response_serialization() {
        let response = types::BranchResponse {
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
        let response = types::MergeResponse {
            merge_request_id: "mr-1".to_string(),
            status: "pending".to_string(),
            requires_approval: true,
            conflicts: Some(vec![types::ConflictResponse {
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
        let tree = types::BranchTreeResponse {
            root: types::BranchNodeResponse {
                id: "root".to_string(),
                name: "main".to_string(),
                status: "completed".to_string(),
                children: vec![types::BranchNodeResponse {
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
        let id = BranchId::new(String::new());
        assert!(matches!(id, Err(BranchError::Internal(_))));
    }

    #[test]
    fn test_merge_request_id_validation() {
        let id = MergeRequestId::new("mr-123".to_string());
        assert!(id.is_ok());

        let id = MergeRequestId::new(String::new());
        assert!(matches!(id, Err(BranchError::Internal(_))));
    }

    // Test default values
    #[test]
    fn test_default_execution_strategy() {
        let default = types::ExecutionStrategyRequest::default();
        assert!(matches!(
            default,
            types::ExecutionStrategyRequest::Sequential
        ));
    }

    #[test]
    fn test_default_merge_strategy() {
        assert_eq!(types::default_merge_strategy(), "union");
    }

    // Test BranchManager
    #[tokio::test]
    async fn test_branch_manager_create_and_get() {
        let manager = BranchManager::new();

        let config = types::BranchConfigRequest {
            name: "test-branch".to_string(),
            agents: vec![],
            execution_strategy: types::ExecutionStrategyRequest::Sequential,
            auto_merge: false,
            merge_strategy: "union".to_string(),
        };

        let branch = manager
            .create_branch(
                config.name.clone(),
                vec![],
                ExecutionStrategy::Sequential,
                config.auto_merge,
                config.merge_strategy.clone(),
            )
            .unwrap();

        assert_eq!(branch.name, "test-branch");
        assert!(matches!(branch.status, BranchStatus::Pending));

        // Get the branch
        let retrieved = manager.get_branch(&branch.id).unwrap();
        assert_eq!(retrieved.name, branch.name);
    }

    #[tokio::test]
    async fn test_branch_manager_list() {
        let manager = BranchManager::new();

        let config = types::BranchConfigRequest {
            name: "branch-1".to_string(),
            agents: vec![],
            execution_strategy: types::ExecutionStrategyRequest::Sequential,
            auto_merge: false,
            merge_strategy: "union".to_string(),
        };

        manager
            .create_branch(
                config.name.clone(),
                vec![],
                ExecutionStrategy::Sequential,
                config.auto_merge,
                config.merge_strategy.clone(),
            )
            .unwrap();

        let branches = manager.list_branches(None, None).unwrap();
        assert_eq!(branches.len(), 1);
        assert_eq!(branches[0].name, "branch-1");
    }

    #[tokio::test]
    async fn test_branch_manager_delete() {
        let manager = BranchManager::new();

        let config = types::BranchConfigRequest {
            name: "to-delete".to_string(),
            agents: vec![],
            execution_strategy: types::ExecutionStrategyRequest::Sequential,
            auto_merge: false,
            merge_strategy: "union".to_string(),
        };

        let branch = manager
            .create_branch(
                config.name.clone(),
                vec![],
                ExecutionStrategy::Sequential,
                config.auto_merge,
                config.merge_strategy.clone(),
            )
            .unwrap();

        manager.delete_branch(&branch.id).unwrap();

        let result = manager.get_branch(&branch.id);
        assert!(matches!(result, Err(BranchError::BranchNotFound(_))));
    }

    #[tokio::test]
    async fn test_merge_request_workflow() {
        let manager = BranchManager::new();

        let config = types::BranchConfigRequest {
            name: "merge-test".to_string(),
            agents: vec![],
            execution_strategy: types::ExecutionStrategyRequest::Sequential,
            auto_merge: false,
            merge_strategy: "union".to_string(),
        };

        let branch = manager
            .create_branch(
                config.name.clone(),
                vec![],
                ExecutionStrategy::Sequential,
                config.auto_merge,
                config.merge_strategy.clone(),
            )
            .unwrap();

        let merge_req = types::MergeRequest {
            strategy: "union".to_string(),
            requires_approval: true,
        };

        let mr = manager
            .request_merge(
                &branch.id,
                merge_req.strategy.clone(),
                merge_req.requires_approval,
            )
            .unwrap();
        assert_eq!(mr.status, MergeRequestStatus::Pending);
        assert!(mr.requires_approval);

        // Approve
        let approved = manager
            .approve_merge(&mr.id, "test-approver".to_string())
            .unwrap();
        assert_eq!(approved.status, MergeRequestStatus::Approved);
    }

    // Test error conversions
    #[test]
    fn test_api_error_into_response() {
        use axum::response::IntoResponse;
        let error = handlers::ApiError::InvalidBranchId("bad-id".to_string());
        let response = error.into_response();
        assert_eq!(response.status(), axum::http::StatusCode::BAD_REQUEST);

        let error = handlers::ApiError::Branch(BranchError::BranchNotFound("id".to_string()));
        let response = error.into_response();
        assert_eq!(response.status(), axum::http::StatusCode::NOT_FOUND);

        let error = handlers::ApiError::Branch(BranchError::MaxBranchesExceeded {
            current: 10,
            limit: 10,
        });
        let response = error.into_response();
        assert_eq!(response.status(), axum::http::StatusCode::CONFLICT);
    }

    // Test router creation
    #[test]
    fn test_routes_creates_valid_router() {
        let _router = routes::routes();
        // Just verify it doesn't panic and has the expected routes
        // Full integration testing would require spinning up a test server
    }
}
