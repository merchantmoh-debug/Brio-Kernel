use config::{Config, ConfigError, Environment};
use secrecy::SecretString;
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
}

/// Sandbox settings for controlling allowed paths.
#[derive(Debug, Deserialize, Clone, Default)]
pub struct SandboxSettings {
    /// Paths that are allowed in the sandbox.
    #[serde(default)]
    pub allowed_paths: Vec<String>,
}

/// Server binding settings.
#[derive(Debug, Deserialize, Clone)]
pub struct ServerSettings {
    /// Host address to bind to.
    pub host: String,
    /// Port to listen on.
    pub port: u16,
}

/// Telemetry configuration settings.
#[derive(Debug, Deserialize, Clone)]
pub struct TelemetrySettings {
    /// Service name for telemetry.
    pub service_name: String,
    /// OTLP endpoint for traces.
    pub otlp_endpoint: Option<String>,
    /// Sampling ratio for traces.
    #[serde(default = "default_sampling")]
    pub sampling_ratio: f64,
}

fn default_sampling() -> f64 {
    1.0
}

/// Database connection settings.
#[derive(Debug, Deserialize, Clone)]
pub struct DatabaseSettings {
    /// Database connection URL.
    pub url: SecretString,
}

/// Mesh networking settings.
#[derive(Debug, Deserialize, Clone)]
pub struct MeshSettings {
    /// Unique identifier for this node.
    pub node_id: Option<String>,
    /// Port to listen on for mesh connections.
    pub port: Option<u16>,
}

/// Inference provider settings.
#[derive(Debug, Deserialize, Clone)]
pub struct InferenceSettings {
    /// `OpenAI` API key.
    pub openai_api_key: Option<SecretString>,
    /// `Anthropic` API key.
    pub anthropic_api_key: Option<SecretString>,
    /// Base URL for `OpenAI` API.
    pub openai_base_url: Option<String>,
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
