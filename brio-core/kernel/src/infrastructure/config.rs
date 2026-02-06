use config::{Config, ConfigError, Environment};
use secrecy::SecretString;
use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct Settings {
    pub server: ServerSettings,
    pub telemetry: TelemetrySettings,
    pub database: DatabaseSettings,
    pub mesh: Option<MeshSettings>,
    pub inference: Option<InferenceSettings>,
    #[serde(default)]
    pub sandbox: SandboxSettings,
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct SandboxSettings {
    #[serde(default)]
    pub allowed_paths: Vec<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ServerSettings {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Deserialize, Clone)]
pub struct TelemetrySettings {
    pub service_name: String,
    pub otlp_endpoint: Option<String>,
    #[serde(default = "default_sampling")]
    pub sampling_ratio: f64,
}

fn default_sampling() -> f64 {
    1.0
}

#[derive(Debug, Deserialize, Clone)]
pub struct DatabaseSettings {
    pub url: SecretString,
}

#[derive(Debug, Deserialize, Clone)]
pub struct MeshSettings {
    pub node_id: Option<String>,
    pub port: Option<u16>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct InferenceSettings {
    pub openai_api_key: Option<SecretString>,
    pub anthropic_api_key: Option<SecretString>,
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
