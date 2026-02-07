//! REST API for the Brio kernel.
//!
//! This module provides HTTP endpoints for managing branches, agents,
//! and other kernel operations.

pub mod branches;

pub use branches::routes as branch_routes;
