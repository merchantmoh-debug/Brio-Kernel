//! WebSocket upgrade handler.

use axum::{
    extract::{State, ws::WebSocketUpgrade},
    response::Response,
};
use tracing::info;

use crate::ws::broadcaster::Broadcaster;
use crate::ws::connection::Connection;

/// Handles WebSocket upgrade requests.
///
/// # Arguments
///
/// * `ws` - The WebSocket upgrade request.
/// * `broadcaster` - The broadcaster state.
pub async fn handle_ws_upgrade(
    ws: WebSocketUpgrade,
    State(broadcaster): State<Broadcaster>,
) -> Response {
    info!("WebSocket upgrade requested");
    ws.on_upgrade(move |socket| async move {
        let receiver = broadcaster.subscribe();
        let connection = Connection::new(socket, receiver);

        if let Err(e) = connection.run().await {
            tracing::error!(error = %e, "WebSocket connection error");
        }
    })
}

/// Creates a router with WebSocket handling.
///
/// # Arguments
///
/// * `broadcaster` - The broadcaster to use for WebSocket connections.
///
/// # Returns
///
/// An Axum router with WebSocket support.
pub fn ws_router(broadcaster: Broadcaster) -> axum::Router {
    axum::Router::new()
        .route("/ws", axum::routing::get(handle_ws_upgrade))
        .with_state(broadcaster)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ws_router_creates_valid_router() {
        let broadcaster = Broadcaster::new();
        let _router = ws_router(broadcaster);
    }
}
