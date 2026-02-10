//! REST API for the Brio kernel.
//!
//! This module provides HTTP endpoints for managing branches, sessions,
//! agents, and other kernel operations.

pub mod branches;
pub mod sessions;

pub use branches::ApiError;
pub use branches::routes as branch_routes;
pub use sessions::routes as session_routes;
