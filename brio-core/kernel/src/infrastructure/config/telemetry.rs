//! Telemetry configuration for the Brio kernel.
//!
//! This module defines OpenTelemetry and observability settings.

use serde::Deserialize;

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

pub(super) fn default_sampling() -> f64 {
    1.0
}
