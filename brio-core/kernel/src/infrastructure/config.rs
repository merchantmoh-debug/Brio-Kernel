use config::{Config, ConfigError, Environment};
use secrecy::SecretString;
use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct Settings {
    pub server: ServerSettings,
    pub telemetry: TelemetrySettings,
    pub database: DatabaseSettings,
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

impl Settings {
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
    pub fn to_socket_addr(&self) -> String {
        format!("{}:{}", self.0, self.1)
    }
}
