use anyhow::Context;
use brio_kernel::host::BrioHostState;
use brio_kernel::infrastructure::{audit, config::Settings, server, telemetry::TelemetryBuilder};
use secrecy::ExposeSecret;
use std::sync::Arc;
use tokio::signal;
use tracing::{error, info};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = Arc::new(Settings::new().context("Failed to load configuration")?);

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
        .context("Failed to initialize telemetry")?;

    info!("Brio Kernel Starting...");
    audit::log_audit(&audit::AuditEvent::SystemStartup {
        component: "Kernel".into(),
    });

    // Initialize Wasmtime Engine
    let engine_config = brio_kernel::engine::linker::create_engine_config();
    let engine =
        wasmtime::Engine::new(&engine_config).context("Failed to create Wasmtime engine")?;

    // Initialize Plugin Registry
    let mut plugin_registry = brio_kernel::registry::PluginRegistry::new(engine);
    let plugins_dir = std::env::current_dir().unwrap_or_default().join("plugins");

    // Scan for plugins
    if let Err(e) = plugin_registry.load_from_directory(&plugins_dir).await {
        error!("Failed to load plugins from {:?}: {:?}", plugins_dir, e);
    } else {
        let plugins = plugin_registry.list_plugins();
        info!("Loaded {} plugins from {:?}", plugins.len(), plugins_dir);
        for p in plugins {
            info!(" - Plugin: {} ({:?})", p.id, p.path);
        }
    }
    let plugin_registry = std::sync::Arc::new(plugin_registry);

    let db_url = config.database.url.expose_secret();

    let openai_key = config
        .inference
        .as_ref()
        .and_then(|i| i.openai_api_key.clone())
        .unwrap_or_else(|| secrecy::SecretString::new("sk-placeholder".into()));

    let openai_base = config
        .inference
        .as_ref()
        .and_then(|i| i.openai_base_url.clone())
        .unwrap_or("https://openrouter.ai/api/v1/".to_string());

    let provider_config = brio_kernel::inference::OpenAIConfig::new(
        openai_key,
        reqwest::Url::parse(&openai_base).context("Invalid OpenAI Base URL")?,
    );
    let provider = brio_kernel::inference::OpenAIProvider::new(provider_config);

    // Create registry (common for both modes)
    let registry = brio_kernel::inference::ProviderRegistry::new();
    registry.register_arc("default", std::sync::Arc::new(provider));
    registry.set_default("default");

    // Check for distributed config
    let mesh_config = config.mesh.clone();
    let node_id = mesh_config
        .as_ref()
        .and_then(|m| m.node_id.clone())
        .map(brio_kernel::mesh::types::NodeId::from);
    let mesh_port = mesh_config
        .as_ref()
        .and_then(|m| m.port)
        .map_or_else(|| "50051".to_string(), |p| p.to_string());

    let state = if let Some(ref id) = node_id {
        info!("Initializing in Distributed Mode (Node ID: {})", id);
        std::sync::Arc::new(
            BrioHostState::new_distributed(
                db_url,
                registry,
                Some(plugin_registry.clone()),
                id.clone(),
                config.sandbox.clone(),
            )
            .await
            .context("Failed to initialize distributed host state")?,
        )
    } else {
        info!("Initializing in Standalone Mode");
        std::sync::Arc::new(
            BrioHostState::new(
                db_url,
                registry,
                Some(plugin_registry.clone()),
                config.sandbox.clone(),
            )
            .await
            .context("Failed to initialize host state")?,
        )
    };

    // Start gRPC server if distributed
    if let Some(id) = node_id {
        let state_clone = state.clone();
        let port = mesh_port.clone();
        tokio::spawn(async move {
            let addr_str = format!("0.0.0.0:{port}");
            let addr = match addr_str.parse() {
                Ok(a) => a,
                Err(e) => {
                    error!("Invalid mesh address '{}': {}", addr_str, e);
                    return;
                }
            };
            let service = brio_kernel::mesh::service::MeshService::new(state_clone, id);

            info!("Mesh gRPC server listening on {}", addr);

            if let Err(e) = tonic::transport::Server::builder()
                .add_service(
                    brio_kernel::mesh::grpc::mesh_transport_server::MeshTransportServer::new(
                        service,
                    ),
                )
                .serve(addr)
                .await
            {
                error!("Mesh gRPC server failed: {:?}", e);
            }
        });
    }

    let broadcaster = state.broadcaster().clone();
    tokio::spawn(async move {
        if let Err(e) = server::run_server(&config, broadcaster).await {
            error!("Control Plane failed: {:?}", e);
        }
    });

    info!("Brio Kernel Initialized. Waiting for shutdown signal...");

    shutdown_signal().await;

    info!("Shutdown signal received, cleaning up...");
    audit::log_audit(&audit::AuditEvent::SystemShutdown {
        reason: "Signal received".into(),
    });

    info!("Brio Kernel Shutdown Complete.");
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        if let Err(e) = signal::ctrl_c().await {
            error!("failed to install Ctrl+C handler: {}", e);
        }
    };

    #[cfg(unix)]
    let terminate = async {
        match signal::unix::signal(signal::unix::SignalKind::terminate()) {
            Ok(mut sig) => {
                sig.recv().await;
            }
            Err(e) => {
                error!("failed to install signal handler: {}", e);
                // Hang instead of panicking to avoid crashing if signal handler fails
                std::future::pending::<()>().await;
            }
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => {},
        () = terminate => {},
    }
}
