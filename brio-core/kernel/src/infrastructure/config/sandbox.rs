//! Sandbox configuration for the Brio kernel.
//!
//! This module defines filesystem sandbox security settings.

use serde::Deserialize;

/// Sandbox settings for controlling allowed paths.
#[derive(Debug, Deserialize, Clone, Default)]
pub struct SandboxSettings {
    /// Paths that are allowed in the sandbox.
    #[serde(default)]
    pub allowed_paths: Vec<String>,
}
