use anyhow::{Context, Result};
use opentelemetry::trace::TracerProvider;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::{propagation::TraceContextPropagator, trace::Sampler, Resource};
use opentelemetry_semantic_conventions::resource;
use tracing_subscriber::{
    fmt::{self, format::FmtSpan},
    layer::SubscriberExt,
    util::SubscriberInitExt,
    EnvFilter, Layer, Registry,
};

/// Builder for setting up telemetry (Logging, Tracing, Metrics).
pub struct TelemetryBuilder {
    service_name: String,
    service_version: String,
    enable_tracing: bool,
    enable_metrics: bool,
    otlp_endpoint: Option<String>,
    log_level: String,
    sampling_ratio: f64,
}

impl TelemetryBuilder {
    pub fn new(service_name: impl Into<String>, service_version: impl Into<String>) -> Self {
        Self {
            service_name: service_name.into(),
            service_version: service_version.into(),
            enable_tracing: false,
            enable_metrics: false,
            otlp_endpoint: None,
            log_level: "info".to_string(),
            sampling_ratio: 1.0,
        }
    }

    #[must_use]
    pub fn with_tracing(mut self, endpoint: impl Into<String>) -> Self {
        self.enable_tracing = true;
        self.otlp_endpoint = Some(endpoint.into());
        self
    }

    #[must_use]
    pub fn with_metrics(mut self) -> Self {
        self.enable_metrics = true;
        self
    }

    #[must_use]
    pub fn with_log_level(mut self, level: impl Into<String>) -> Self {
        self.log_level = level.into();
        self
    }

    #[must_use]
    pub fn with_sampling_ratio(mut self, ratio: f64) -> Self {
        self.sampling_ratio = ratio;
        self
    }

    /// Initializes the telemetry system with configured exporters.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The OTLP span exporter cannot be built
    /// - The tracing subscriber cannot be initialized
    pub fn init(self) -> Result<()> {
        opentelemetry::global::set_text_map_propagator(TraceContextPropagator::new());

        let env_filter =
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(&self.log_level));

        let fmt_layer = fmt::layer().json().with_span_events(FmtSpan::CLOSE).boxed();

        let registry = Registry::default().with(env_filter).with(fmt_layer);

        if self.enable_tracing {
            if let Some(endpoint) = self.otlp_endpoint {
                let resource = Resource::builder()
                    .with_attributes(vec![
                        opentelemetry::KeyValue::new(
                            resource::SERVICE_NAME,
                            self.service_name.clone(),
                        ),
                        opentelemetry::KeyValue::new(
                            resource::SERVICE_VERSION,
                            self.service_version.clone(),
                        ),
                    ])
                    .build();

                let exporter = opentelemetry_otlp::SpanExporter::builder()
                    .with_tonic()
                    .with_endpoint(endpoint)
                    .build()
                    .context("Failed to build OTLP span exporter")?;

                let processor =
                    opentelemetry_sdk::trace::BatchSpanProcessor::builder(exporter).build();

                let provider = opentelemetry_sdk::trace::SdkTracerProvider::builder()
                    .with_span_processor(processor)
                    .with_resource(resource)
                    .with_sampler(Sampler::ParentBased(Box::new(Sampler::TraceIdRatioBased(
                        self.sampling_ratio,
                    ))))
                    .build();

                opentelemetry::global::set_tracer_provider(provider.clone());

                let tracer = provider.tracer("brio-kernel");

                let telemetry_layer = tracing_opentelemetry::layer().with_tracer(tracer);

                registry
                    .with(telemetry_layer)
                    .try_init()
                    .context("Failed to init subscriber")?;
            } else {
                registry.try_init().context("Failed to init subscriber")?;
            }
        } else {
            registry.try_init().context("Failed to init subscriber")?;
        }

        Ok(())
    }
}
