//! Configuration management for the Brio kernel.
//!
//! This module provides structured configuration for various
//! domains including server, database, telemetry, mesh networking,
//! inference providers, sandbox policies, and branching orchestration.
//!
//! # Example
//!
//! ```
//! use brio_kernel::infrastructure::config::Settings;
//!
//! let settings = Settings::new().expect("Failed to load configuration");
//! ```

pub mod branching;
pub mod database;
pub mod inference;
pub mod mesh;
pub mod sandbox;
pub mod server;
pub mod telemetry;

// Re-export all config types for backward compatibility
pub use branching::BranchingSettings;
pub use database::DatabaseSettings;
pub use inference::InferenceSettings;
pub use mesh::MeshSettings;
pub use sandbox::SandboxSettings;
pub use server::ServerSettings;
pub use telemetry::TelemetrySettings;

use config::{Config, ConfigError, Environment};
use serde::Deserialize;

/// Top-level configuration for the Brio kernel.
#[derive(Debug, Deserialize, Clone)]
pub struct Settings {
    /// Server settings.
    pub server: ServerSettings,
    /// Telemetry settings.
    pub telemetry: TelemetrySettings,
    /// Database settings.
    pub database: DatabaseSettings,
    /// Mesh networking settings.
    pub mesh: Option<MeshSettings>,
    /// Inference provider settings.
    pub inference: Option<InferenceSettings>,
    /// Sandbox settings.
    #[serde(default)]
    pub sandbox: SandboxSettings,
    /// Branching orchestrator settings.
    #[serde(default)]
    pub branching: BranchingSettings,
}

impl Settings {
    /// Creates a new settings instance from environment variables and defaults.
    ///
    /// # Errors
    ///
    /// Returns an error if the configuration cannot be built or deserialized.
    pub fn new() -> Result<Self, ConfigError> {
        let _run_mode = std::env::var("BRIO_ENV").unwrap_or_else(|_| "development".into());

        let s = Config::builder()
            // Start with default values
            .set_default("server.host", "127.0.0.1")?
            .set_default("server.port", 9090)?
            .set_default("telemetry.service_name", "brio-kernel")?
            .set_default("telemetry.sampling_ratio", 1.0)?
            // Merge in Environment variables
            .add_source(Environment::with_prefix("BRIO").separator("__"))
            .build()?;

        s.try_deserialize()
    }
}

/// Helper for strong typing addresses
pub struct BindAddress(pub String, pub u16);

impl BindAddress {
    /// Converts the bind address to a `SocketAddr`.
    ///
    /// # Errors
    ///
    /// Returns an error if the IP address string cannot be parsed.
    pub fn to_socket_addr(&self) -> anyhow::Result<std::net::SocketAddr> {
        let ip = self
            .0
            .parse()
            .map_err(|e| anyhow::anyhow!("Invalid IP address '{}': {e}", self.0))?;
        Ok(std::net::SocketAddr::new(ip, self.1))
    }
}
