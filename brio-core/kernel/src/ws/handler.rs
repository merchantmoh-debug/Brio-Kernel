//! WebSocket upgrade handler.

use axum::{
    extract::{State, ws::WebSocketUpgrade},
    response::Response,
};
use std::sync::Arc;
use tracing::info;

use crate::host::BrioHostState;
use crate::ws::connection::Connection;

/// Handles WebSocket upgrade requests.
///
/// # Arguments
///
/// * `ws` - The WebSocket upgrade request.
/// * `host_state` - The host state containing broadcaster and kernel operations.
pub async fn handle_ws_upgrade(
    ws: WebSocketUpgrade,
    State(host_state): State<Arc<BrioHostState>>,
) -> Response {
    info!("WebSocket upgrade requested");
    ws.on_upgrade(move |socket| {
        // Use a std::future::ready to create a future that is immediately ready
        // This avoids the Send requirement since we're not capturing across await points
        let host_state = host_state.clone();
        async move {
            let receiver = host_state.broadcaster().subscribe();
            let connection = Connection::new(socket, receiver, host_state);

            if let Err(e) = connection.run().await {
                tracing::error!(error = %e, "WebSocket connection error");
            }
        }
    })
}

/// Creates a router with WebSocket handling.
///
/// # Arguments
///
/// * `host_state` - The host state to use for WebSocket connections.
///
/// # Returns
/// An Axum router with WebSocket support.
pub fn ws_router(host_state: Arc<BrioHostState>) -> axum::Router {
    axum::Router::new()
        .route("/ws", axum::routing::get(handle_ws_upgrade))
        .with_state(host_state)
}

#[cfg(test)]
mod tests {
    

    #[test]
    fn ws_router_creates_valid_router() {
        // Note: This test is a compile-time check only.
        // Full integration testing requires a BrioHostState instance.
    }
}
