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

    init_telemetry(&config).context("Failed to initialize telemetry")?;
    log_startup();

    let plugin_registry = init_plugin_registry().await?;
    let registry = init_inference_provider(&config)?;
    let state = init_host_state(&config, registry, &plugin_registry).await?;

    start_mesh_server(&config, &state);
    start_control_plane(&config, &state);

    info!("Brio Kernel Initialized. Waiting for shutdown signal...");
    shutdown_signal().await;
    graceful_shutdown();

    Ok(())
}

fn init_telemetry(config: &Settings) -> anyhow::Result<()> {
    let mut builder = TelemetryBuilder::new("brio-kernel", "0.1.0")
        .with_log_level("debug")
        .with_sampling_ratio(config.telemetry.sampling_ratio);

    if let Some(ref endpoint) = config.telemetry.otlp_endpoint {
        builder = builder.with_tracing(endpoint);
    }

    builder.with_metrics().init()
}

fn log_startup() {
    info!("Brio Kernel Starting...");
    audit::log_audit(&audit::AuditEvent::SystemStartup {
        component: "Kernel".into(),
    });
}

async fn init_plugin_registry()
-> anyhow::Result<std::sync::Arc<brio_kernel::registry::PluginRegistry>> {
    let engine_config = brio_kernel::engine::linker::create_engine_config();
    let engine = wasmtime::Engine::new(&engine_config)?;
    let mut registry = brio_kernel::registry::PluginRegistry::new(engine);
    let plugins_dir = std::env::current_dir().unwrap_or_default().join("plugins");

    if let Err(e) = registry.load_from_directory(&plugins_dir).await {
        error!("Failed to load plugins from {:?}: {:?}", plugins_dir, e);
    } else {
        let plugins = registry.list_plugins();
        info!("Loaded {} plugins from {:?}", plugins.len(), plugins_dir);
        for p in &plugins {
            info!(" - Plugin: {} ({:?})", p.id, p.path);
        }
    }

    Ok(std::sync::Arc::new(registry))
}

fn init_inference_provider(
    config: &Settings,
) -> anyhow::Result<brio_kernel::inference::ProviderRegistry> {
    let openai_key = config
        .inference
        .as_ref()
        .and_then(|i| i.openai_api_key.clone())
        .unwrap_or_else(|| secrecy::SecretString::new("sk-placeholder".into()));

    let openai_base = config
        .inference
        .as_ref()
        .and_then(|i| i.openai_base_url.clone())
        .unwrap_or_else(|| "https://openrouter.ai/api/v1/".to_string());

    let provider_config = brio_kernel::inference::OpenAIConfig::new(
        openai_key,
        reqwest::Url::parse(&openai_base).context("Invalid OpenAI Base URL")?,
    );
    let provider = brio_kernel::inference::OpenAIProvider::new(provider_config);

    let registry = brio_kernel::inference::ProviderRegistry::new();
    registry.register_arc("default", std::sync::Arc::new(provider));
    registry.set_default("default");

    Ok(registry)
}

async fn init_host_state(
    config: &Settings,
    registry: brio_kernel::inference::ProviderRegistry,
    plugin_registry: &std::sync::Arc<brio_kernel::registry::PluginRegistry>,
) -> anyhow::Result<std::sync::Arc<BrioHostState>> {
    let db_url = config.database.url.expose_secret();

    let state = if let Some(ref node_id) = config.mesh.as_ref().and_then(|m| m.node_id.clone()) {
        let id = brio_kernel::mesh::types::NodeId::from(node_id.clone());
        info!("Initializing in Distributed Mode (Node ID: {})", id);
        BrioHostState::new_distributed(
            db_url,
            registry,
            Some(plugin_registry.clone()),
            id,
            config.sandbox.clone(),
        )
        .await
        .context("Failed to initialize distributed host state")?
    } else {
        info!("Initializing in Standalone Mode");
        BrioHostState::new(
            db_url,
            registry,
            Some(plugin_registry.clone()),
            config.sandbox.clone(),
        )
        .await
        .context("Failed to initialize host state")?
    };

    Ok(std::sync::Arc::new(state))
}

fn start_mesh_server(config: &Settings, state: &std::sync::Arc<BrioHostState>) {
    let Some(node_id) = config.mesh.as_ref().and_then(|m| m.node_id.clone()) else {
        return;
    };

    let state_clone = state.clone();
    let port = config.mesh.as_ref().and_then(|m| m.port).unwrap_or(50051);

    tokio::spawn(async move {
        let addr_str = format!("0.0.0.0:{port}");
        let Ok(addr) = addr_str.parse() else {
            error!("Invalid mesh address: {}", addr_str);
            return;
        };

        let id = brio_kernel::mesh::types::NodeId::from(node_id);
        let service = brio_kernel::mesh::service::MeshService::new(state_clone, id);

        info!("Mesh gRPC server listening on {}", addr);

        if let Err(e) = tonic::transport::Server::builder()
            .add_service(
                brio_kernel::mesh::grpc::mesh_transport_server::MeshTransportServer::new(service),
            )
            .serve(addr)
            .await
        {
            error!("Mesh gRPC server failed: {:?}", e);
        }
    });
}

fn start_control_plane(config: &Settings, state: &std::sync::Arc<BrioHostState>) {
    let broadcaster = state.broadcaster().clone();
    let config_clone = config.clone();

    tokio::spawn(async move {
        if let Err(e) = server::run_server(&config_clone, broadcaster).await {
            error!("Control Plane failed: {:?}", e);
        }
    });
}

fn graceful_shutdown() {
    info!("Shutdown signal received, cleaning up...");
    audit::log_audit(&audit::AuditEvent::SystemShutdown {
        reason: "Signal received".into(),
    });
    info!("Brio Kernel Shutdown Complete.");
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
