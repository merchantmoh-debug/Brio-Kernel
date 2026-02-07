//! Integration tests for branching orchestrator
//!
//! These tests verify the end-to-end functionality of the branching system,
//! including branch lifecycle, parallel execution, limits, merging, and recovery.

use std::path::PathBuf;

use supervisor::branch::manager::BranchError;
use supervisor::domain::{
    AgentAssignment, BranchConfig, BranchId, BranchSource, BranchStatus,
    ExecutionMetrics, ExecutionStrategy, Priority,
};

mod common;
use common::TestContext;

// ============= Branch Lifecycle Tests =============

#[tokio::test]
async fn test_branch_lifecycle() {
    let ctx = TestContext::new();
    let manager_arc = ctx.branch_manager();
    let mut manager = manager_arc.lock().unwrap();

    // 1. Create branch
    let branch_id = manager
        .create_branch(
            BranchSource::Base(PathBuf::from("./test_files")),
            BranchConfig {
                name: "Test Branch".to_string(),
                agents: vec![],
                execution_strategy: ExecutionStrategy::Sequential,
                auto_merge: false,
                merge_strategy: "union".to_string(),
            },
        )
        .await
        .unwrap();

    // 2. Verify branch created
    let branch = manager.get_branch(branch_id).unwrap().unwrap();
    assert_eq!(branch.name(), "Test Branch");
    assert!(matches!(branch.status(), BranchStatus::Pending));

    // 3. Mark as executing
    manager.mark_executing(branch_id, 1).unwrap();
    let branch = manager.get_branch(branch_id).unwrap().unwrap();
    assert!(matches!(branch.status(), BranchStatus::Active));

    // 4. Complete branch
    let result = supervisor::domain::BranchResult {
        branch_id,
        file_changes: vec![],
        agent_results: vec![],
        metrics: ExecutionMetrics {
            total_duration_ms: 100,
            files_processed: 0,
            agents_executed: 1,
            peak_memory_bytes: 0,
        },
    };
    manager.complete_branch(branch_id, result).unwrap();

    // 5. Verify completed
    let branch = manager.get_branch(branch_id).unwrap().unwrap();
    assert!(matches!(branch.status(), BranchStatus::Completed));
}

#[tokio::test]
async fn test_branch_creation_validation() {
    let ctx = TestContext::new();
    let manager_arc = ctx.branch_manager();
    let mut manager = manager_arc.lock().unwrap();

    // Test empty name
    let result = manager
        .create_branch(
            BranchSource::Base(PathBuf::from("./test_files")),
            BranchConfig {
                name: "".to_string(),
                agents: vec![],
                execution_strategy: ExecutionStrategy::Sequential,
                auto_merge: false,
                merge_strategy: "union".to_string(),
            },
        )
        .await;

    assert!(result.is_err());
}

#[tokio::test]
async fn test_branch_from_parent() {
    let ctx = TestContext::new();
    let manager_arc = ctx.branch_manager();
    let mut manager = manager_arc.lock().unwrap();

    // Create parent branch
    let parent_id = manager
        .create_branch(
            BranchSource::Base(PathBuf::from("./test_files")),
            BranchConfig {
                name: "Parent Branch".to_string(),
                agents: vec![],
                execution_strategy: ExecutionStrategy::Sequential,
                auto_merge: false,
                merge_strategy: "union".to_string(),
            },
        )
        .await
        .unwrap();

    // Complete parent
    manager.mark_executing(parent_id, 1).unwrap();
    let result = supervisor::domain::BranchResult {
        branch_id: parent_id,
        file_changes: vec![],
        agent_results: vec![],
        metrics: ExecutionMetrics {
            total_duration_ms: 100,
            files_processed: 0,
            agents_executed: 1,
            peak_memory_bytes: 0,
        },
    };
    manager.complete_branch(parent_id, result).unwrap();

    // Create child from parent
    let child_id = manager
        .create_branch(
            BranchSource::Branch(parent_id),
            BranchConfig {
                name: "Child Branch".to_string(),
                agents: vec![],
                execution_strategy: ExecutionStrategy::Sequential,
                auto_merge: false,
                merge_strategy: "union".to_string(),
            },
        )
        .await
        .unwrap();

    let child = manager.get_branch(child_id).unwrap().unwrap();
    assert_eq!(child.parent_id(), Some(parent_id));
}

// ============= Max Branch Limit Tests =============

#[tokio::test]
async fn test_max_branch_limit() {
    let ctx = TestContext::new();
    let manager_arc = ctx.branch_manager();
    let mut manager = manager_arc.lock().unwrap();

    // Create 8 branches (max)
    for i in 0..8 {
        let result = manager
            .create_branch(
                BranchSource::Base(PathBuf::from("./test_files")),
                BranchConfig {
                    name: format!("Branch {}", i),
                    agents: vec![],
                    execution_strategy: ExecutionStrategy::Sequential,
                    auto_merge: false,
                    merge_strategy: "union".to_string(),
                },
            )
            .await;
        assert!(result.is_ok(), "Should be able to create branch {}", i);
    }

    // 9th should fail
    let result = manager
        .create_branch(
            BranchSource::Base(PathBuf::from("./test_files")),
            BranchConfig {
                name: "Over limit".to_string(),
                agents: vec![],
                execution_strategy: ExecutionStrategy::Sequential,
                auto_merge: false,
                merge_strategy: "union".to_string(),
            },
        )
        .await;

    assert!(matches!(
        result,
        Err(BranchError::MaxBranchesExceeded { current: 8, limit: 8 })
    ));
}

#[tokio::test]
async fn test_completed_branches_dont_count_towards_limit() {
    let ctx = TestContext::new();
    let manager_arc = ctx.branch_manager();
    let mut manager = manager_arc.lock().unwrap();

    // Create and complete a branch
    let branch_id = manager
        .create_branch(
            BranchSource::Base(PathBuf::from("./test_files")),
            BranchConfig {
                name: "To Complete".to_string(),
                agents: vec![],
                execution_strategy: ExecutionStrategy::Sequential,
                auto_merge: false,
                merge_strategy: "union".to_string(),
            },
        )
        .await
        .unwrap();

    manager.mark_executing(branch_id, 1).unwrap();
    let result = supervisor::domain::BranchResult {
        branch_id,
        file_changes: vec![],
        agent_results: vec![],
        metrics: ExecutionMetrics {
            total_duration_ms: 100,
            files_processed: 0,
            agents_executed: 1,
            peak_memory_bytes: 0,
        },
    };
    manager.complete_branch(branch_id, result).unwrap();

    // Should be able to create another branch (total still 1 active, not 8)
    let result = manager
        .create_branch(
            BranchSource::Base(PathBuf::from("./test_files")),
            BranchConfig {
                name: "New Branch".to_string(),
                agents: vec![],
                execution_strategy: ExecutionStrategy::Sequential,
                auto_merge: false,
                merge_strategy: "union".to_string(),
            },
        )
        .await;

    assert!(result.is_ok());
}

// ============= Merge Tests =============

#[tokio::test]
async fn test_merge_request_with_approval() {
    let ctx = TestContext::new();
    let manager_arc = ctx.branch_manager();
    let mut manager = manager_arc.lock().unwrap();

    // Create and complete branch
    let branch_id = manager
        .create_branch(
            BranchSource::Base(PathBuf::from("./test_files")),
            BranchConfig {
                name: "To Merge".to_string(),
                agents: vec![],
                execution_strategy: ExecutionStrategy::Sequential,
                auto_merge: false,
                merge_strategy: "union".to_string(),
            },
        )
        .await
        .unwrap();

    manager.mark_executing(branch_id, 1).unwrap();
    let result = supervisor::domain::BranchResult {
        branch_id,
        file_changes: vec![],
        agent_results: vec![],
        metrics: ExecutionMetrics {
            total_duration_ms: 100,
            files_processed: 0,
            agents_executed: 1,
            peak_memory_bytes: 0,
        },
    };
    manager.complete_branch(branch_id, result).unwrap();

    // Request merge with approval required
    let merge_req = manager
        .request_merge(branch_id, "union", true)
        .await
        .unwrap();

    // Approve and execute
    manager.approve_merge(merge_req, "test_user").unwrap();

    // Verify merge can proceed
    let branch = manager.get_branch(branch_id).unwrap().unwrap();
    assert!(matches!(branch.status(), BranchStatus::Completed));
}

#[tokio::test]
async fn test_merge_requires_completed_status() {
    let ctx = TestContext::new();
    let manager_arc = ctx.branch_manager();
    let mut manager = manager_arc.lock().unwrap();

    // Create branch but don't complete it
    let branch_id = manager
        .create_branch(
            BranchSource::Base(PathBuf::from("./test_files")),
            BranchConfig {
                name: "Incomplete".to_string(),
                agents: vec![],
                execution_strategy: ExecutionStrategy::Sequential,
                auto_merge: false,
                merge_strategy: "union".to_string(),
            },
        )
        .await
        .unwrap();

    // Try to request merge
    let result = manager.request_merge(branch_id, "union", false).await;

    assert!(matches!(
        result,
        Err(BranchError::InvalidBranchState { expected, actual, .. })
        if expected == "Completed" && actual == "Pending"
    ));
}

#[tokio::test]
async fn test_invalid_merge_strategy() {
    let ctx = TestContext::new();
    let manager_arc = ctx.branch_manager();
    let mut manager = manager_arc.lock().unwrap();

    // Create and complete branch
    let branch_id = manager
        .create_branch(
            BranchSource::Base(PathBuf::from("./test_files")),
            BranchConfig {
                name: "To Merge".to_string(),
                agents: vec![],
                execution_strategy: ExecutionStrategy::Sequential,
                auto_merge: false,
                merge_strategy: "union".to_string(),
            },
        )
        .await
        .unwrap();

    manager.mark_executing(branch_id, 1).unwrap();
    let result = supervisor::domain::BranchResult {
        branch_id,
        file_changes: vec![],
        agent_results: vec![],
        metrics: ExecutionMetrics {
            total_duration_ms: 100,
            files_processed: 0,
            agents_executed: 1,
            peak_memory_bytes: 0,
        },
    };
    manager.complete_branch(branch_id, result).unwrap();

    // Request merge with invalid strategy
    let result = manager.request_merge(branch_id, "invalid-strategy", false).await;

    assert!(matches!(
        result,
        Err(BranchError::InvalidStrategy(strategy))
        if strategy == "invalid-strategy"
    ));
}

// ============= Nested Branches Tests =============

#[tokio::test]
async fn test_nested_branches() {
    let ctx = TestContext::new();
    let manager_arc = ctx.branch_manager();
    let mut manager = manager_arc.lock().unwrap();

    // Create parent branch
    let parent_id = manager
        .create_branch(
            BranchSource::Base(PathBuf::from("./test_files")),
            BranchConfig {
                name: "Parent".to_string(),
                agents: vec![],
                execution_strategy: ExecutionStrategy::Sequential,
                auto_merge: false,
                merge_strategy: "union".to_string(),
            },
        )
        .await
        .unwrap();

    // Complete parent so we can branch from it
    manager.mark_executing(parent_id, 1).unwrap();
    let result = supervisor::domain::BranchResult {
        branch_id: parent_id,
        file_changes: vec![],
        agent_results: vec![],
        metrics: ExecutionMetrics {
            total_duration_ms: 100,
            files_processed: 0,
            agents_executed: 1,
            peak_memory_bytes: 0,
        },
    };
    manager.complete_branch(parent_id, result).unwrap();

    // Create child branches
    let child1 = manager
        .create_branch(
            BranchSource::Branch(parent_id),
            BranchConfig {
                name: "Child 1".to_string(),
                agents: vec![],
                execution_strategy: ExecutionStrategy::Sequential,
                auto_merge: false,
                merge_strategy: "union".to_string(),
            },
        )
        .await
        .unwrap();

    let child2 = manager
        .create_branch(
            BranchSource::Branch(parent_id),
            BranchConfig {
                name: "Child 2".to_string(),
                agents: vec![],
                execution_strategy: ExecutionStrategy::Sequential,
                auto_merge: false,
                merge_strategy: "union".to_string(),
            },
        )
        .await
        .unwrap();

    // Verify tree structure
    let tree = manager.get_branch_tree(parent_id).unwrap();
    assert_eq!(tree.branch.id(), parent_id);
    assert_eq!(tree.children.len(), 2);

    let child_ids: Vec<BranchId> = tree.children.iter().map(|c| c.branch.id()).collect();
    assert!(child_ids.contains(&child1));
    assert!(child_ids.contains(&child2));
}

#[tokio::test]
async fn test_branch_tree_depth() {
    let ctx = TestContext::new();
    let manager_arc = ctx.branch_manager();
    let mut manager = manager_arc.lock().unwrap();

    // Create nested branches: root -> child -> grandchild
    let root = manager
        .create_branch(
            BranchSource::Base(PathBuf::from("./test_files")),
            BranchConfig {
                name: "Root".to_string(),
                agents: vec![],
                execution_strategy: ExecutionStrategy::Sequential,
                auto_merge: false,
                merge_strategy: "union".to_string(),
            },
        )
        .await
        .unwrap();

    // Complete root
    manager.mark_executing(root, 1).unwrap();
    let result = supervisor::domain::BranchResult {
        branch_id: root,
        file_changes: vec![],
        agent_results: vec![],
        metrics: ExecutionMetrics {
            total_duration_ms: 100,
            files_processed: 0,
            agents_executed: 1,
            peak_memory_bytes: 0,
        },
    };
    manager.complete_branch(root, result).unwrap();

    // Create child
    let child = manager
        .create_branch(
            BranchSource::Branch(root),
            BranchConfig {
                name: "Child".to_string(),
                agents: vec![],
                execution_strategy: ExecutionStrategy::Sequential,
                auto_merge: false,
                merge_strategy: "union".to_string(),
            },
        )
        .await
        .unwrap();

    // Complete child
    manager.mark_executing(child, 1).unwrap();
    let result = supervisor::domain::BranchResult {
        branch_id: child,
        file_changes: vec![],
        agent_results: vec![],
        metrics: ExecutionMetrics {
            total_duration_ms: 100,
            files_processed: 0,
            agents_executed: 1,
            peak_memory_bytes: 0,
        },
    };
    manager.complete_branch(child, result).unwrap();

    // Create grandchild
    let grandchild = manager
        .create_branch(
            BranchSource::Branch(child),
            BranchConfig {
                name: "Grandchild".to_string(),
                agents: vec![],
                execution_strategy: ExecutionStrategy::Sequential,
                auto_merge: false,
                merge_strategy: "union".to_string(),
            },
        )
        .await
        .unwrap();

    // Verify tree depth
    let tree = manager.get_branch_tree(root).unwrap();
    assert_eq!(tree.total_nodes(), 3);

    // Verify nesting level
    let child_node = tree.children.iter().find(|c| c.branch.id() == child).unwrap();
    assert_eq!(child_node.children.len(), 1);
    assert_eq!(child_node.children[0].branch.id(), grandchild);
}

// ============= Recovery Tests =============

#[tokio::test]
async fn test_branch_recovery_after_restart() {
    let ctx = TestContext::new();
    let manager_arc = ctx.branch_manager();
    let mut manager = manager_arc.lock().unwrap();

    // Create branches
    let id1 = manager
        .create_branch(
            BranchSource::Base(PathBuf::from("./test_files")),
            BranchConfig {
                name: "Branch 1".to_string(),
                agents: vec![],
                execution_strategy: ExecutionStrategy::Sequential,
                auto_merge: false,
                merge_strategy: "union".to_string(),
            },
        )
        .await
        .unwrap();

    let id2 = manager
        .create_branch(
            BranchSource::Base(PathBuf::from("./test_files")),
            BranchConfig {
                name: "Branch 2".to_string(),
                agents: vec![],
                execution_strategy: ExecutionStrategy::Sequential,
                auto_merge: false,
                merge_strategy: "union".to_string(),
            },
        )
        .await
        .unwrap();

    // Mark one as executing
    manager.mark_executing(id1, 1).unwrap();

    // Simulate restart - recover branches
    let recovered = manager.recover_branches().unwrap();

    // Should recover both
    assert_eq!(recovered.len(), 2);
    assert!(recovered.contains(&id1));
    assert!(recovered.contains(&id2));

    // Executing branch should be reset to Pending
    let branch = manager.get_branch(id1).unwrap().unwrap();
    assert!(matches!(branch.status(), BranchStatus::Pending));
}

// ============= Status Transition Tests =============

#[tokio::test]
async fn test_invalid_status_transitions() {
    let ctx = TestContext::new();
    let manager_arc = ctx.branch_manager();
    let mut manager = manager_arc.lock().unwrap();

    // Create branch
    let branch_id = manager
        .create_branch(
            BranchSource::Base(PathBuf::from("./test_files")),
            BranchConfig {
                name: "Test".to_string(),
                agents: vec![],
                execution_strategy: ExecutionStrategy::Sequential,
                auto_merge: false,
                merge_strategy: "union".to_string(),
            },
        )
        .await
        .unwrap();

    // Cannot complete without executing first
    let result = manager.complete_branch(
        branch_id,
        supervisor::domain::BranchResult {
            branch_id,
            file_changes: vec![],
            agent_results: vec![],
            metrics: ExecutionMetrics {
                total_duration_ms: 100,
                files_processed: 0,
                agents_executed: 1,
                peak_memory_bytes: 0,
            },
        },
    );

    assert!(result.is_err());
}

#[tokio::test]
async fn test_terminal_status_is_terminal() {
    let ctx = TestContext::new();
    let manager_arc = ctx.branch_manager();
    let mut manager = manager_arc.lock().unwrap();

    // Create branch
    let branch_id = manager
        .create_branch(
            BranchSource::Base(PathBuf::from("./test_files")),
            BranchConfig {
                name: "To Complete".to_string(),
                agents: vec![],
                execution_strategy: ExecutionStrategy::Sequential,
                auto_merge: false,
                merge_strategy: "union".to_string(),
            },
        )
        .await
        .unwrap();

    // Complete it
    manager.mark_executing(branch_id, 1).unwrap();
    let result = supervisor::domain::BranchResult {
        branch_id,
        file_changes: vec![],
        agent_results: vec![],
        metrics: ExecutionMetrics {
            total_duration_ms: 100,
            files_processed: 0,
            agents_executed: 1,
            peak_memory_bytes: 0,
        },
    };
    manager.complete_branch(branch_id, result).unwrap();

    // Verify it's terminal
    let branch = manager.get_branch(branch_id).unwrap().unwrap();
    assert!(branch.status().is_terminal());
    assert!(!branch.is_active());
}

// ============= Configuration Tests =============

#[tokio::test]
async fn test_branch_with_agents() {
    let ctx = TestContext::new();
    let manager_arc = ctx.branch_manager();
    let mut manager = manager_arc.lock().unwrap();

    let agent = AgentAssignment::new("agent_coder", None, Priority::new(100)).unwrap();

    let branch_id = manager
        .create_branch(
            BranchSource::Base(PathBuf::from("./test_files")),
            BranchConfig {
                name: "With Agents".to_string(),
                agents: vec![agent],
                execution_strategy: ExecutionStrategy::Sequential,
                auto_merge: false,
                merge_strategy: "union".to_string(),
            },
        )
        .await
        .unwrap();

    let branch = manager.get_branch(branch_id).unwrap().unwrap();
    let config: BranchConfig = serde_json::from_str(branch.config()).unwrap();
    assert_eq!(config.agents.len(), 1);
    assert_eq!(config.agents[0].agent_id.as_str(), "agent_coder");
}

#[tokio::test]
async fn test_branch_with_parallel_strategy() {
    let ctx = TestContext::new();
    let manager_arc = ctx.branch_manager();
    let mut manager = manager_arc.lock().unwrap();

    let agents = vec![
        AgentAssignment::new("agent1", None, Priority::new(100)).unwrap(),
        AgentAssignment::new("agent2", None, Priority::new(100)).unwrap(),
        AgentAssignment::new("agent3", None, Priority::new(100)).unwrap(),
    ];

    let branch_id = manager
        .create_branch(
            BranchSource::Base(PathBuf::from("./test_files")),
            BranchConfig {
                name: "Parallel".to_string(),
                agents,
                execution_strategy: ExecutionStrategy::Parallel { max_concurrent: 2 },
                auto_merge: false,
                merge_strategy: "union".to_string(),
            },
        )
        .await
        .unwrap();

    let branch = manager.get_branch(branch_id).unwrap().unwrap();
    let config: BranchConfig = serde_json::from_str(branch.config()).unwrap();
    assert_eq!(config.execution_strategy.concurrency_limit(), 2);
}

// ============= Error Handling Tests =============

#[tokio::test]
async fn test_get_nonexistent_branch() {
    let ctx = TestContext::new();
    let manager_arc = ctx.branch_manager();
    let manager = manager_arc.lock().unwrap();

    let result = manager.get_branch(BranchId::new());
    assert!(result.is_ok());
    assert!(result.unwrap().is_none());
}

#[tokio::test]
async fn test_update_nonexistent_branch() {
    let ctx = TestContext::new();
    let manager_arc = ctx.branch_manager();
    let manager = manager_arc.lock().unwrap();

    let result = manager.update_status(BranchId::new(), BranchStatus::Active);
    assert!(matches!(result, Err(BranchError::BranchNotFound(_))));
}

#[tokio::test]
async fn test_approve_nonexistent_merge() {
    let ctx = TestContext::new();
    let manager_arc = ctx.branch_manager();
    let manager = manager_arc.lock().unwrap();

    let merge_id = supervisor::merge::MergeId::new();
    let result = manager.approve_merge(merge_id, "user");
    assert!(result.is_err());
}

// ============= Edge Cases =============

#[tokio::test]
async fn test_branch_name_length_limits() {
    let ctx = TestContext::new();
    let manager_arc = ctx.branch_manager();
    let mut manager = manager_arc.lock().unwrap();

    // Test max length name (256 chars)
    let long_name = "a".repeat(256);
    let result = manager
        .create_branch(
            BranchSource::Base(PathBuf::from("./test_files")),
            BranchConfig {
                name: long_name,
                agents: vec![],
                execution_strategy: ExecutionStrategy::Sequential,
                auto_merge: false,
                merge_strategy: "union".to_string(),
            },
        )
        .await;
    assert!(result.is_ok());

    // Test name too long (257 chars)
    let too_long = "a".repeat(257);
    let result = manager
        .create_branch(
            BranchSource::Base(PathBuf::from("./test_files")),
            BranchConfig {
                name: too_long,
                agents: vec![],
                execution_strategy: ExecutionStrategy::Sequential,
                auto_merge: false,
                merge_strategy: "union".to_string(),
            },
        )
        .await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_branch_with_task_override() {
    let ctx = TestContext::new();
    let manager_arc = ctx.branch_manager();
    let mut manager = manager_arc.lock().unwrap();

    let agent = AgentAssignment::new(
        "agent_coder",
        Some("Custom task override".to_string()),
        Priority::new(150),
    )
    .unwrap();

    let branch_id = manager
        .create_branch(
            BranchSource::Base(PathBuf::from("./test_files")),
            BranchConfig {
                name: "With Override".to_string(),
                agents: vec![agent],
                execution_strategy: ExecutionStrategy::Sequential,
                auto_merge: false,
                merge_strategy: "union".to_string(),
            },
        )
        .await
        .unwrap();

    let branch = manager.get_branch(branch_id).unwrap().unwrap();
    let config: BranchConfig = serde_json::from_str(branch.config()).unwrap();
    assert_eq!(config.agents[0].task_override, Some("Custom task override".to_string()));
}
