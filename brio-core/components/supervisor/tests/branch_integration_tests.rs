//! Integration tests for branching orchestrator
//!
//! These tests verify the end-to-end functionality of the branching system,
//! including branch lifecycle, parallel execution, limits, merging, and recovery.

use std::path::PathBuf;

use supervisor::branch::BranchError;
use supervisor::branch::BranchSource;
use supervisor::domain::{
    AgentAssignment, BranchConfig, BranchId, BranchStatus, ExecutionStrategy, Priority,
};

mod common;
use common::{DEFAULT_MERGE_STRATEGY, TEST_FILES_DIR, TestContext};

// Helper function to run async branch creation synchronously
fn create_branch_sync(
    manager: &mut supervisor::branch::BranchManager,
    source: BranchSource,
    config: BranchConfig,
) -> Result<BranchId, supervisor::branch::BranchError> {
    tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current()
            .block_on(async { manager.create_branch(source, config).await })
    })
}

// Helper function to run async request_merge synchronously
fn request_merge_sync(
    manager: &mut supervisor::branch::BranchManager,
    branch_id: BranchId,
    strategy: &str,
    requires_approval: bool,
) -> Result<supervisor::merge::MergeId, supervisor::branch::BranchError> {
    tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(async {
            manager
                .request_merge(branch_id, strategy, requires_approval)
                .await
        })
    })
}

// ============= Branch Lifecycle Tests =============

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_branch_lifecycle() {
    let ctx = TestContext::new();
    let manager_arc = ctx.branch_manager();
    let mut manager = manager_arc.lock().unwrap();

    // 1. Create branch
    let branch_id = create_branch_sync(
        &mut manager,
        BranchSource::Base(PathBuf::from(TEST_FILES_DIR)),
        TestContext::default_test_config("Test Branch"),
    )
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
    let result = TestContext::default_test_result(branch_id);
    manager.complete_branch(branch_id, result).unwrap();

    // 5. Verify completed
    let branch = manager.get_branch(branch_id).unwrap().unwrap();
    assert!(matches!(branch.status(), BranchStatus::Completed));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_branch_creation_validation() {
    let ctx = TestContext::new();
    let manager_arc = ctx.branch_manager();
    let _manager = manager_arc.lock().unwrap();

    // Test empty name - should fail validation
    let config_result = BranchConfig::new(
        "",
        vec![],
        ExecutionStrategy::Sequential,
        false,
        DEFAULT_MERGE_STRATEGY,
    );
    assert!(config_result.is_err());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_branch_from_parent() {
    let ctx = TestContext::new();
    let manager_arc = ctx.branch_manager();
    let mut manager = manager_arc.lock().unwrap();

    // Create parent branch
    let parent_id = create_branch_sync(
        &mut manager,
        BranchSource::Base(PathBuf::from(TEST_FILES_DIR)),
        TestContext::default_test_config("Parent Branch"),
    )
    .unwrap();

    // Complete parent
    manager.mark_executing(parent_id, 1).unwrap();
    let result = TestContext::default_test_result(parent_id);
    manager.complete_branch(parent_id, result).unwrap();

    // Create child from parent
    let child_id = create_branch_sync(
        &mut manager,
        BranchSource::Branch(parent_id),
        TestContext::default_test_config("Child Branch"),
    )
    .unwrap();

    let child = manager.get_branch(child_id).unwrap().unwrap();
    assert_eq!(child.parent_id(), Some(parent_id));
}

// ============= Max Branch Limit Tests =============

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_max_branch_limit() {
    let ctx = TestContext::new();
    let manager_arc = ctx.branch_manager();
    let mut manager = manager_arc.lock().unwrap();

    // Create 8 branches (max)
    for i in 0..8 {
        let result = create_branch_sync(
            &mut manager,
            BranchSource::Base(PathBuf::from(TEST_FILES_DIR)),
            TestContext::default_test_config(format!("Branch {i}")),
        );
        assert!(result.is_ok(), "Should be able to create branch {i}");
    }

    // 9th should fail
    let result = create_branch_sync(
        &mut manager,
        BranchSource::Base(PathBuf::from(TEST_FILES_DIR)),
        TestContext::default_test_config("Over limit"),
    );

    assert!(matches!(
        result,
        Err(BranchError::MaxBranchesExceeded {
            current: 8,
            limit: 8
        })
    ));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_completed_branches_dont_count_towards_limit() {
    let ctx = TestContext::new();
    let manager_arc = ctx.branch_manager();
    let mut manager = manager_arc.lock().unwrap();

    // Create and complete a branch
    let branch_id = create_branch_sync(
        &mut manager,
        BranchSource::Base(PathBuf::from(TEST_FILES_DIR)),
        TestContext::default_test_config("To Complete"),
    )
    .unwrap();

    manager.mark_executing(branch_id, 1).unwrap();
    let result = TestContext::default_test_result(branch_id);
    manager.complete_branch(branch_id, result).unwrap();

    // Should be able to create another branch (total still 1 active, not 8)
    let result = create_branch_sync(
        &mut manager,
        BranchSource::Base(PathBuf::from(TEST_FILES_DIR)),
        TestContext::default_test_config("New Branch"),
    );

    assert!(result.is_ok());
}

// ============= Merge Tests =============

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_merge_request_with_approval() {
    let ctx = TestContext::new();
    let manager_arc = ctx.branch_manager();
    let mut manager = manager_arc.lock().unwrap();

    // Create and complete branch
    let branch_id = create_branch_sync(
        &mut manager,
        BranchSource::Base(PathBuf::from(TEST_FILES_DIR)),
        TestContext::default_test_config("To Merge"),
    )
    .unwrap();

    manager.mark_executing(branch_id, 1).unwrap();
    let result = TestContext::default_test_result(branch_id);
    manager.complete_branch(branch_id, result).unwrap();

    // Request merge with approval required
    let merge_req =
        request_merge_sync(&mut manager, branch_id, DEFAULT_MERGE_STRATEGY, true).unwrap();

    // Approve and execute
    manager.approve_merge(merge_req, "test_user").unwrap();

    // Verify merge can proceed
    let branch = manager.get_branch(branch_id).unwrap().unwrap();
    assert!(matches!(branch.status(), BranchStatus::Completed));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_merge_requires_completed_status() {
    let ctx = TestContext::new();
    let manager_arc = ctx.branch_manager();
    let mut manager = manager_arc.lock().unwrap();

    // Create branch but don't complete it
    let branch_id = create_branch_sync(
        &mut manager,
        BranchSource::Base(PathBuf::from(TEST_FILES_DIR)),
        TestContext::default_test_config("Incomplete"),
    )
    .unwrap();

    // Try to request merge
    let result = request_merge_sync(&mut manager, branch_id, DEFAULT_MERGE_STRATEGY, false);

    assert!(matches!(
        result,
        Err(BranchError::InvalidBranchState { expected, actual, .. })
        if expected == "Completed" && actual == "Pending"
    ));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_invalid_merge_strategy() {
    let ctx = TestContext::new();
    let manager_arc = ctx.branch_manager();
    let mut manager = manager_arc.lock().unwrap();

    // Create and complete branch
    let branch_id = create_branch_sync(
        &mut manager,
        BranchSource::Base(PathBuf::from(TEST_FILES_DIR)),
        TestContext::default_test_config("To Merge"),
    )
    .unwrap();

    manager.mark_executing(branch_id, 1).unwrap();
    let result = TestContext::default_test_result(branch_id);
    manager.complete_branch(branch_id, result).unwrap();

    // Request merge with invalid strategy
    let result = request_merge_sync(&mut manager, branch_id, "invalid-strategy", false);

    assert!(matches!(
        result,
        Err(BranchError::InvalidStrategy(strategy))
        if strategy == "invalid-strategy"
    ));
}

// ============= Nested Branches Tests =============

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_nested_branches() {
    let ctx = TestContext::new();
    let manager_arc = ctx.branch_manager();
    let mut manager = manager_arc.lock().unwrap();

    // Create parent branch
    let parent_id = create_branch_sync(
        &mut manager,
        BranchSource::Base(PathBuf::from(TEST_FILES_DIR)),
        TestContext::default_test_config("Parent"),
    )
    .unwrap();

    // Complete parent so we can branch from it
    manager.mark_executing(parent_id, 1).unwrap();
    let result = TestContext::default_test_result(parent_id);
    manager.complete_branch(parent_id, result).unwrap();

    // Create child branches
    let child1 = create_branch_sync(
        &mut manager,
        BranchSource::Branch(parent_id),
        TestContext::default_test_config("Child 1"),
    )
    .unwrap();

    let child2 = create_branch_sync(
        &mut manager,
        BranchSource::Branch(parent_id),
        TestContext::default_test_config("Child 2"),
    )
    .unwrap();

    // Verify tree structure
    let tree = manager.get_branch_tree(parent_id).unwrap();
    assert_eq!(tree.branch.id(), parent_id);
    assert_eq!(tree.children.len(), 2);

    let child_ids: Vec<BranchId> = tree.children.iter().map(|c| c.branch.id()).collect();
    assert!(child_ids.contains(&child1));
    assert!(child_ids.contains(&child2));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_branch_tree_depth() {
    let ctx = TestContext::new();
    let manager_arc = ctx.branch_manager();
    let mut manager = manager_arc.lock().unwrap();

    // Create nested branches: root -> child -> grandchild
    let root = create_branch_sync(
        &mut manager,
        BranchSource::Base(PathBuf::from(TEST_FILES_DIR)),
        TestContext::default_test_config("Root"),
    )
    .unwrap();

    // Complete root
    manager.mark_executing(root, 1).unwrap();
    let result = TestContext::default_test_result(root);
    manager.complete_branch(root, result).unwrap();

    // Create child
    let child = create_branch_sync(
        &mut manager,
        BranchSource::Branch(root),
        TestContext::default_test_config("Child"),
    )
    .unwrap();

    // Complete child
    manager.mark_executing(child, 1).unwrap();
    let result = TestContext::default_test_result(child);
    manager.complete_branch(child, result).unwrap();

    // Create grandchild
    let grandchild = create_branch_sync(
        &mut manager,
        BranchSource::Branch(child),
        TestContext::default_test_config("Grandchild"),
    )
    .unwrap();

    // Verify tree depth
    let tree = manager.get_branch_tree(root).unwrap();
    assert_eq!(tree.total_nodes(), 3);

    // Verify nesting level
    let child_node = tree
        .children
        .iter()
        .find(|c| c.branch.id() == child)
        .unwrap();
    assert_eq!(child_node.children.len(), 1);
    assert_eq!(child_node.children[0].branch.id(), grandchild);
}

// ============= Recovery Tests =============

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_branch_recovery_after_restart() {
    let ctx = TestContext::new();
    let manager_arc = ctx.branch_manager();
    let mut manager = manager_arc.lock().unwrap();

    // Create branches
    let id1 = create_branch_sync(
        &mut manager,
        BranchSource::Base(PathBuf::from(TEST_FILES_DIR)),
        TestContext::default_test_config("Branch 1"),
    )
    .unwrap();

    let id2 = create_branch_sync(
        &mut manager,
        BranchSource::Base(PathBuf::from(TEST_FILES_DIR)),
        TestContext::default_test_config("Branch 2"),
    )
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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_invalid_status_transitions() {
    let ctx = TestContext::new();
    let manager_arc = ctx.branch_manager();
    let mut manager = manager_arc.lock().unwrap();

    // Create branch
    let branch_id = create_branch_sync(
        &mut manager,
        BranchSource::Base(PathBuf::from(TEST_FILES_DIR)),
        TestContext::default_test_config("Test"),
    )
    .unwrap();

    // Cannot complete without executing first
    let result = manager.complete_branch(branch_id, TestContext::default_test_result(branch_id));

    assert!(result.is_err());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_terminal_status_is_terminal() {
    let ctx = TestContext::new();
    let manager_arc = ctx.branch_manager();
    let mut manager = manager_arc.lock().unwrap();

    // Create branch
    let branch_id = create_branch_sync(
        &mut manager,
        BranchSource::Base(PathBuf::from(TEST_FILES_DIR)),
        TestContext::default_test_config("To Complete"),
    )
    .unwrap();

    // Complete it
    manager.mark_executing(branch_id, 1).unwrap();
    let result = TestContext::default_test_result(branch_id);
    manager.complete_branch(branch_id, result).unwrap();

    // Verify it's terminal
    let branch = manager.get_branch(branch_id).unwrap().unwrap();
    assert!(branch.status().is_terminal());
    assert!(!branch.is_active());
}

// ============= Configuration Tests =============

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_branch_with_agents() {
    let ctx = TestContext::new();
    let manager_arc = ctx.branch_manager();
    let mut manager = manager_arc.lock().unwrap();

    let agent = AgentAssignment::new("agent_coder", None, Priority::new(100)).unwrap();

    let branch_id = create_branch_sync(
        &mut manager,
        BranchSource::Base(PathBuf::from(TEST_FILES_DIR)),
        BranchConfig::new(
            "With Agents",
            vec![agent],
            ExecutionStrategy::Sequential,
            false,
            DEFAULT_MERGE_STRATEGY,
        )
        .unwrap(),
    )
    .unwrap();

    let branch = manager.get_branch(branch_id).unwrap().unwrap();
    let config = branch.config();
    assert_eq!(config.agents().len(), 1);
    assert_eq!(config.agents()[0].agent_id().as_str(), "agent_coder");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_branch_with_parallel_strategy() {
    let ctx = TestContext::new();
    let manager_arc = ctx.branch_manager();
    let mut manager = manager_arc.lock().unwrap();

    let agents = vec![
        AgentAssignment::new("agent1", None, Priority::new(100)).unwrap(),
        AgentAssignment::new("agent2", None, Priority::new(100)).unwrap(),
        AgentAssignment::new("agent3", None, Priority::new(100)).unwrap(),
    ];

    let branch_id = create_branch_sync(
        &mut manager,
        BranchSource::Base(PathBuf::from("./test_files")),
        BranchConfig::new(
            "Parallel",
            agents,
            ExecutionStrategy::Parallel { max_concurrent: 2 },
            false,
            "union",
        )
        .unwrap(),
    )
    .unwrap();

    let branch = manager.get_branch(branch_id).unwrap().unwrap();
    let config = branch.config();
    assert_eq!(config.execution_strategy().concurrency_limit(), 2);
}

// ============= Error Handling Tests =============

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_get_nonexistent_branch() {
    let ctx = TestContext::new();
    let manager_arc = ctx.branch_manager();
    let manager = manager_arc.lock().unwrap();

    let result = manager.get_branch(BranchId::new());
    assert!(result.is_ok());
    assert!(result.unwrap().is_none());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_update_nonexistent_branch() {
    let ctx = TestContext::new();
    let manager_arc = ctx.branch_manager();
    let manager = manager_arc.lock().unwrap();

    let result = manager.update_status(BranchId::new(), BranchStatus::Active);
    assert!(matches!(result, Err(BranchError::BranchNotFound(_))));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_approve_nonexistent_merge() {
    let ctx = TestContext::new();
    let manager_arc = ctx.branch_manager();
    let manager = manager_arc.lock().unwrap();

    let merge_id = supervisor::merge::MergeId::new();
    let result = manager.approve_merge(merge_id, "user");
    assert!(result.is_err());
}

// ============= Edge Cases =============

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_branch_name_length_limits() {
    let ctx = TestContext::new();
    let manager_arc = ctx.branch_manager();
    let mut manager = manager_arc.lock().unwrap();

    // Test max length name (256 chars) - should succeed
    let long_name = "a".repeat(256);
    let result = create_branch_sync(
        &mut manager,
        BranchSource::Base(PathBuf::from(TEST_FILES_DIR)),
        TestContext::default_test_config(long_name),
    );
    assert!(result.is_ok());

    // Test name too long (257 chars) - should fail validation
    let too_long = "a".repeat(257);
    let config_result = BranchConfig::new(
        too_long,
        vec![],
        ExecutionStrategy::Sequential,
        false,
        DEFAULT_MERGE_STRATEGY,
    );
    assert!(config_result.is_err());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
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

    let branch_id = create_branch_sync(
        &mut manager,
        BranchSource::Base(PathBuf::from(TEST_FILES_DIR)),
        BranchConfig::new(
            "With Override",
            vec![agent],
            ExecutionStrategy::Sequential,
            false,
            DEFAULT_MERGE_STRATEGY,
        )
        .unwrap(),
    )
    .unwrap();

    let branch = manager.get_branch(branch_id).unwrap().unwrap();
    let config = branch.config();
    assert_eq!(
        config.agents()[0].task_override(),
        Some("Custom task override")
    );
}
