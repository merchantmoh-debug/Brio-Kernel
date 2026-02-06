//! Brio Kernel - Core library for the Brio system.
//!
//! This crate provides the core functionality for the Brio kernel,
//! including WebAssembly component management, inference providers,
//! virtual file system, and distributed mesh networking.

// TODO: Gradually add documentation and upgrade to deny
#![warn(missing_docs)]
#![warn(clippy::pedantic)]

/// WebAssembly component engine and runtime.
pub mod engine;
/// Host state and WIT interface implementations.
pub mod host;
/// LLM inference providers and registry.
pub mod inference;
/// Infrastructure components (config, server, telemetry).
pub mod infrastructure;
/// Distributed mesh networking.
pub mod mesh;
/// Plugin registry and management.
pub mod registry;
/// SQL store and query policy.
pub mod store;
/// Virtual file system for sandboxed operations.
pub mod vfs;
/// WebSocket broadcaster for real-time updates.
pub mod ws;
