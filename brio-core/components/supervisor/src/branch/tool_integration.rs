//! Branch Tool Integration Example
//!
//! This module shows how to integrate the CreateBranchTool with the Supervisor's
//! BranchManager. This bridges the gap between the agent-sdk tools and the
//! supervisor domain.
//!
//! ## Integration Pattern
//!
//! The supervisor implements the callbacks that the CreateBranchTool uses.
//! This keeps the agent-sdk independent while allowing full supervisor functionality.

use std::sync::{Arc, Mutex};

use agent_sdk::agent::tools::{
    BranchCreationCallback, BranchCreationConfig, BranchCreationResult, BranchId as ToolBranchId,
    BranchInfo, BranchToolError, CreateBranchTool, ListBranchesTool,
};
use agent_sdk::{Tool, ToolParser};
use agent_sdk::tools::ToolRegistry;

use crate::branch::{BranchManager, BranchError};
use crate::domain::{BranchConfig, BranchId, BranchStatus};

/// Creates a CreateBranchTool wired to the supervisor's BranchManager.
///
/// # Arguments
///
/// * `branch_manager` - The BranchManager instance from the supervisor
///
/// # Example
///
/// ```rust,no_run
/// use supervisor::branch::BranchManager;
/// use supervisor::integration::create_branch_tool_for_supervisor;
///
/// fn setup_agent_tools(manager: Arc<Mutex<BranchManager>>) {
///     let create_tool = create_branch_tool_for_supervisor(manager);
///     // Register with your agent's ToolRegistry
/// }
/// ```
pub fn create_branch_tool_for_supervisor(
    branch_manager: Arc<Mutex<BranchManager>>,
) -> CreateBranchTool {
    let callback: BranchCreationCallback = Arc::new(move |config: BranchCreationConfig| {
        let manager = branch_manager.lock().map_err(|_| {
            BranchToolError::CreationFailed("Branch manager lock poisoned".to_string())
        })?;

        // Convert parent string to BranchId if provided
        let parent_id: Option<BranchId> = config.parent.as_ref().and_then(|p| {
            // Try to parse as UUID - in production, you'd want better error handling
            uuid::Uuid::parse_str(p).ok().map(BranchId::from_uuid)
        });

        // Create the branch configuration
        let branch_config = BranchConfig::default();

        // Create the branch using the supervisor's manager
        // Note: This is a sync call from within an async context - 
        // in production, consider using tokio::task::block_in_place or channels
        let result = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                manager.create_branch(
                    config.name.clone(),
                    branch_config,
                    parent_id,
                ).await
            })
        });

        let (branch_id, session_id) = result.map_err(|e| match e {
            BranchError::Validation(_) => BranchToolError::InvalidName(config.name),
            BranchError::MaxBranchesReached(_) => BranchToolError::MaxBranchesReached,
            BranchError::BranchNotFound(_) => {
                BranchToolError::ParentNotFound(config.parent.unwrap_or_default())
            }
            _ => BranchToolError::CreationFailed(e.to_string()),
        })?;

        Ok(BranchCreationResult {
            branch_id: ToolBranchId::new(branch_id.to_string()),
            session_id,
            workspace_path: format!("/workspace/{}", config.name),
        })
    });

    CreateBranchTool::new(callback)
}

/// Creates a ListBranchesTool wired to the supervisor's BranchManager.
///
/// # Arguments
///
/// * `branch_manager` - The BranchManager instance from the supervisor
///
/// # Example
///
/// ```rust,no_run
/// use supervisor::branch::BranchManager;
/// use supervisor::integration::create_list_branches_tool_for_supervisor;
///
/// fn setup_agent_tools(manager: Arc<Mutex<BranchManager>>) {
///     let list_tool = create_list_branches_tool_for_supervisor(manager);
///     // Register with your agent's ToolRegistry
/// }
/// ```
pub fn create_list_branches_tool_for_supervisor(
    branch_manager: Arc<Mutex<BranchManager>>,
) -> ListBranchesTool {
    let callback = Arc::new(move || {
        let manager = branch_manager.lock().map_err(|_| {
            BranchToolError::CreationFailed("Branch manager lock poisoned".to_string())
        })?;

        // Get list of branches - in production, you'd want pagination/filtering
        // This is a simplified example
        let branches = vec![]; // Placeholder - you'd call manager.list_branches() if it existed

        Ok(branches)
    });

    ListBranchesTool::new(callback)
}

/// Helper function to register all branch tools with a ToolRegistry.
///
/// This is a convenience function that registers both create_branch and list_branches
/// tools with the appropriate parsers.
///
/// # Arguments
///
/// * `registry` - The ToolRegistry to register tools with
/// * `branch_manager` - The BranchManager instance from the supervisor
///
/// # Example
///
/// ```rust,no_run
/// use supervisor::branch::BranchManager;
/// use supervisor::integration::register_branch_tools;
/// use agent_sdk::ToolRegistry;
///
/// fn setup_registry(manager: Arc<Mutex<BranchManager>>) {
///     let mut registry = ToolRegistry::new();
///     register_branch_tools(&mut registry, manager);
///     // Now agents can use branch tools
/// }
/// ```
pub fn register_branch_tools(
    registry: &mut ToolRegistry,
    branch_manager: Arc<Mutex<BranchManager>>,
) {
    // Register create_branch tool
    let create_tool = create_branch_tool_for_supervisor(Arc::clone(&branch_manager));
    registry.register(
        "create_branch",
        Box::new(create_tool),
        agent_sdk::agent::parsers::create_create_branch_parser(),
    );

    // Register list_branches tool
    let list_tool = create_list_branches_tool_for_supervisor(branch_manager);
    registry.register(
        "list_branches",
        Box::new(list_tool),
        agent_sdk::agent::parsers::create_list_branches_parser(),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: These tests would need a mock BranchManager to work properly
    // The actual implementation would require setting up the full supervisor context

    #[test]
    fn test_branch_tool_error_display() {
        let err = BranchToolError::MaxBranchesReached;
        assert!(err.to_string().contains("Maximum"));

        let err = BranchToolError::InvalidName("test".to_string());
        assert!(err.to_string().contains("test"));
    }
}
