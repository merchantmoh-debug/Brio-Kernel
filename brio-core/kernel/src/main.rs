use brio_kernel::host::BrioHostState;
use brio_kernel::infrastructure::{audit, config::Settings, server, telemetry::TelemetryBuilder};
use secrecy::ExposeSecret;
use tokio::signal;
use tracing::{error, info};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = Settings::new().expect("Failed to load configuration");

    let mut telemetry_builder = TelemetryBuilder::new("brio-kernel", "0.1.0")
        .with_log_level("debug")
        .with_sampling_ratio(config.telemetry.sampling_ratio);

    telemetry_builder = if let Some(ref endpoint) = config.telemetry.otlp_endpoint {
        telemetry_builder.with_tracing(endpoint)
    } else {
        telemetry_builder
    };

    telemetry_builder
        .with_metrics()
        .init()
        .expect("Failed to initialize telemetry");

    info!("Brio Kernel Starting...");
    audit::log_audit(audit::AuditEvent::SystemStartup {
        component: "Kernel".into(),
    });

    let db_url = config.database.url.expose_secret();

    // Clean Code: Configure Provider (DIP)
    // In future, this would load from strict configuration.
    // For now we default to a placeholder config or environment.
    // However, to keep it functional, we check env vars or use a default.
    let provider_config = brio_kernel::inference::OpenAIConfig {
        api_key: secrecy::SecretString::new("sk-placeholder".into()), // Placeholder for now
        base_url: reqwest::Url::parse("https://openrouter.ai/api/v1").expect("Invalid URL"),
    };
    let provider = brio_kernel::inference::OpenAIProvider::new(provider_config);

    let state = match BrioHostState::new(db_url, Box::new(provider)).await {
        Ok(s) => s,
        Err(e) => {
            error!("Failed to initialize host state: {:?}", e);
            std::process::exit(1);
        }
    };

    let broadcaster = state.broadcaster().clone();
    let server_config = config.clone();
    tokio::spawn(async move {
        if let Err(e) = server::run_server(&server_config, broadcaster).await {
            error!("Control Plane failed: {:?}", e);
        }
    });

    info!("Brio Kernel Initialized. Waiting for shutdown signal...");

    shutdown_signal().await;

    info!("Shutdown signal received, cleaning up...");
    audit::log_audit(audit::AuditEvent::SystemShutdown {
        reason: "Signal received".into(),
    });

    info!("Brio Kernel Shutdown Complete.");
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}
