use crate::infrastructure::config::Settings;
use crate::ws::{Broadcaster, handler::ws_router};
use axum::{Router, routing::get};
use metrics_exporter_prometheus::PrometheusBuilder;
use std::net::SocketAddr;

#[cfg(unix)]
use pprof::protos::Message;

async fn health_check() -> &'static str {
    "OK"
}

#[cfg(unix)]
async fn pprof_profile() -> impl axum::response::IntoResponse {
    let guard = match pprof::ProfilerGuardBuilder::default()
        .frequency(100)
        .blocklist(&["libc", "libgcc", "pthread", "vdso"])
        .build()
    {
        Ok(g) => g,
        Err(e) => {
            tracing::error!("Failed to start profiler: {:?}", e);
            return (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                [("Content-Type", "text/plain")],
                format!("Failed to start profiler: {e:?}").into_bytes(),
            );
        }
    };

    tokio::time::sleep(std::time::Duration::from_secs(10)).await;

    match guard.report().build() {
        Ok(report) => {
            let mut body = Vec::new();
            match report.pprof() {
                Ok(profile) => {
                    if let Err(e) = profile.encode(&mut body) {
                        tracing::error!("Failed to encode pprof profile: {:?}", e);
                        return (
                            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                            [("Content-Type", "text/plain")],
                            format!("Failed to encode profile: {e:?}").into_bytes(),
                        );
                    }
                    (
                        axum::http::StatusCode::OK,
                        [("Content-Type", "application/octet-stream")],
                        body,
                    )
                }
                Err(e) => {
                    tracing::error!("Failed to generate pprof: {:?}", e);
                    (
                        axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                        [("Content-Type", "text/plain")],
                        format!("Failed to generate pprof: {e:?}").into_bytes(),
                    )
                }
            }
        }
        Err(e) => {
            tracing::error!("Failed to build report: {:?}", e);
            (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                [("Content-Type", "text/plain")],
                format!("Failed to build report: {e:?}").into_bytes(),
            )
        }
    }
}

#[cfg(not(unix))]
async fn pprof_profile() -> impl axum::response::IntoResponse {
    (
        axum::http::StatusCode::NOT_IMPLEMENTED,
        "Profiling is only available on Unix systems",
    )
}

/// Runs the control plane HTTP server with WebSocket support.
///
/// # Errors
///
/// Returns an error if the server fails to start or encounters an error while running.
pub async fn run_server(config: &Settings, broadcaster: Broadcaster) -> anyhow::Result<()> {
    let builder = PrometheusBuilder::new();
    let handle = builder
        .install_recorder()
        .map_err(|e| anyhow::anyhow!("Failed to install Prometheus recorder: {e}"))?;

    let control_plane = Router::new()
        .route("/health/live", get(health_check))
        .route("/health/ready", get(health_check))
        .route("/metrics", get(move || std::future::ready(handle.render())))
        .route("/debug/pprof/profile", get(pprof_profile));

    let app = control_plane.merge(ws_router(broadcaster));

    let addr_str = format!("{}:{}", config.server.host, config.server.port);
    let addr: SocketAddr = addr_str.parse()?;

    tracing::info!("Control Plane listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
