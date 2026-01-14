use crate::infrastructure::config::Settings;
use axum::{Router, routing::get};
use metrics_exporter_prometheus::PrometheusBuilder;
use pprof::protos::Message;
use std::net::SocketAddr;

async fn health_check() -> &'static str {
    "OK"
}

// Handler for pprof CPU profile
async fn pprof_profile() -> impl axum::response::IntoResponse {
    let guard = pprof::ProfilerGuardBuilder::default()
        .frequency(100)
        .blocklist(&["libc", "libgcc", "pthread", "vdso"])
        .build()
        .unwrap();

    // Sleep for a bit to collect profile
    tokio::time::sleep(std::time::Duration::from_secs(10)).await;

    match guard.report().build() {
        Ok(report) => {
            let mut body = Vec::new();
            match report.pprof() {
                Ok(profile) => {
                    profile.encode(&mut body).unwrap();
                    ([("Content-Type", "application/octet-stream")], body)
                }
                Err(e) => {
                    tracing::error!("Failed to generate pprof: {:?}", e);
                    (
                        [("Content-Type", "text/plain")],
                        format!("Failed to generate pprof: {:?}", e).into_bytes(),
                    )
                }
            }
        }
        Err(e) => {
            tracing::error!("Failed to build report: {:?}", e);
            (
                [("Content-Type", "text/plain")],
                format!("Failed to build report: {:?}", e).into_bytes(),
            )
        }
    }
}

pub async fn run_server(config: &Settings) -> anyhow::Result<()> {
    let builder = PrometheusBuilder::new();
    let handle = builder
        .install_recorder()
        .expect("failed to install Prometheus recorder");

    let app = Router::new()
        .route("/health/live", get(health_check))
        .route("/health/ready", get(health_check))
        .route("/metrics", get(move || std::future::ready(handle.render())))
        .route("/debug/pprof/profile", get(pprof_profile));

    let addr_str = format!("{}:{}", config.server.host, config.server.port);
    let addr: SocketAddr = addr_str.parse()?;

    tracing::info!("Control Plane listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
