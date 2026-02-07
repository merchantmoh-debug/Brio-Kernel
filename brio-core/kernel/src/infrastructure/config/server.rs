//! Server configuration for the Brio kernel.
//!
//! This module defines HTTP server binding and runtime settings.

use serde::Deserialize;

/// Server binding settings.
#[derive(Debug, Deserialize, Clone)]
pub struct ServerSettings {
    /// Host address to bind to.
    pub host: String,
    /// Port to listen on.
    pub port: u16,
}
