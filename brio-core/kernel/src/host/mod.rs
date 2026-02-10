//! Host state and WIT interface implementations.
//!
//! This module provides the core host state management, permission checking,
//! and mesh networking functionality for the Brio kernel.

pub mod mesh;
pub mod permissions;
pub mod state;

// Re-export primary types for convenience
pub use mesh::{MeshHandler, MeshRoute, RouteType};
pub use permissions::{
    AllowAllPermissions, PermissionChecker, PermissionError, RestrictedPermissions,
};
pub use state::BrioHostState;
