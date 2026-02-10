//! Provider registry for managing multiple LLM backends.
//!
//! This module provides the [`ProviderRegistry`] which allows concurrent
//! registration and use of multiple LLM providers.

pub mod core;
pub mod routing;

pub use core::ProviderRegistry;
