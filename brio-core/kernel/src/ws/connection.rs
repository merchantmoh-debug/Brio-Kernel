//! WebSocket connection lifecycle management.

use axum::extract::ws::{Message, WebSocket};
use bytes::Bytes;
use futures_util::StreamExt;
use sqlx::{Column, Row};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::interval;
use tracing::{debug, error, info, warn};

use crate::host::BrioHostState;
use crate::ws::broadcaster::BroadcastReceiver;
use crate::ws::types::{
    BroadcastMessage, ClientId, ClientMessage, ClientResponse, SessionAction, WsError,
};

const PING_INTERVAL: Duration = Duration::from_secs(30);

/// A WebSocket connection with a unique client ID.
pub struct Connection {
    client_id: ClientId,
    stream: WebSocket,
    receiver: BroadcastReceiver,
    host_state: Arc<BrioHostState>,
}

impl Connection {
    /// Creates a new WebSocket connection.
    ///
    /// # Arguments
    ///
    /// * `stream` - The WebSocket stream.
    /// * `receiver` - The broadcast receiver for messages.
    /// * `host_state` - The host state for accessing kernel operations.
    pub fn new(
        stream: WebSocket,
        receiver: BroadcastReceiver,
        host_state: Arc<BrioHostState>,
    ) -> Self {
        let client_id = ClientId::generate();
        info!(client_id = %client_id, "WebSocket connection established");
        Self {
            client_id,
            stream,
            receiver,
            host_state,
        }
    }

    /// Returns the client ID for this connection.
    pub fn client_id(&self) -> ClientId {
        self.client_id
    }

    /// Runs the connection loop, handling incoming messages and broadcasts.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - WebSocket communication fails
    /// - Message handling encounters an error
    /// - Broadcast channel is closed
    pub async fn run(mut self) -> Result<(), WsError> {
        let mut ping_interval = interval(PING_INTERVAL);

        loop {
            tokio::select! {
                incoming = self.stream.next() => {
                    match incoming {
                        Some(Ok(msg)) => {
                            if self.handle_incoming_message(msg).await? {
                                break;
                            }
                        }
                        Some(Err(e)) => {
                            error!(client_id = %self.client_id, error = %e, "WebSocket error");
                            return Err(WsError::AxumWs(e));
                        }
                        None => {
                            debug!(client_id = %self.client_id, "Stream ended");
                            break;
                        }
                    }
                }

                broadcast_result = self.receiver.recv() => {
                    match broadcast_result {
                        Ok(msg) => {
                            self.send_broadcast_message(msg).await?;
                        }
                        Err(WsError::ChannelClosed) => {
                            info!(client_id = %self.client_id, "Broadcast channel closed");
                            break;
                        }
                        Err(e) => {
                            warn!(client_id = %self.client_id, error = %e, "Broadcast error");
                        }
                    }
                }

                _ = ping_interval.tick() => {
                    self.send_ping().await?;
                }
            }
        }

        self.graceful_close().await
    }

    async fn handle_incoming_message(&mut self, message: Message) -> Result<bool, WsError> {
        match message {
            Message::Text(text) => {
                debug!(client_id = %self.client_id, len = text.len(), "Received text");

                // Clone the Arc to avoid holding &self across await
                let host_state = Arc::clone(&self.host_state);
                let client_id = self.client_id;

                match handle_client_message(host_state, client_id, &text).await {
                    Ok(response) => {
                        let response_text =
                            serde_json::to_string(&response).map_err(WsError::Serialization)?;
                        self.stream
                            .send(Message::Text(response_text.into()))
                            .await
                            .map_err(WsError::AxumWs)?;
                    }
                    Err(e) => {
                        let error_response = ClientResponse::error(e.to_string());
                        let response_text = serde_json::to_string(&error_response)
                            .map_err(WsError::Serialization)?;
                        self.stream
                            .send(Message::Text(response_text.into()))
                            .await
                            .map_err(WsError::AxumWs)?;
                    }
                }
                Ok(false)
            }
            Message::Binary(data) => {
                debug!(client_id = %self.client_id, len = data.len(), "Received binary");
                warn!(client_id = %self.client_id, "Unexpected binary from client");
                Ok(false)
            }
            Message::Ping(data) => {
                debug!(client_id = %self.client_id, "Ping received");
                self.stream
                    .send(Message::Pong(data))
                    .await
                    .map_err(WsError::AxumWs)?;
                Ok(false)
            }
            Message::Pong(_) => {
                debug!(client_id = %self.client_id, "Pong received");
                Ok(false)
            }
            Message::Close(_) => {
                info!(client_id = %self.client_id, "Client initiated close");
                Ok(true)
            }
        }
    }

    async fn send_broadcast_message(&mut self, message: BroadcastMessage) -> Result<(), WsError> {
        let should_close = matches!(message, BroadcastMessage::Shutdown);
        let payload = message.to_frame_payload()?;

        self.stream
            .send(Message::Text(payload.into()))
            .await
            .map_err(WsError::AxumWs)?;

        if should_close {
            info!(client_id = %self.client_id, "Shutdown broadcast received");
        }

        Ok(())
    }

    async fn send_ping(&mut self) -> Result<(), WsError> {
        debug!(client_id = %self.client_id, "Sending ping");
        self.stream
            .send(Message::Ping(Bytes::new()))
            .await
            .map_err(WsError::AxumWs)
    }

    async fn graceful_close(mut self) -> Result<(), WsError> {
        debug!(client_id = %self.client_id, "Closing gracefully");
        self.stream
            .send(Message::Close(None))
            .await
            .map_err(WsError::AxumWs)?;
        info!(client_id = %self.client_id, "Connection closed");
        Ok(())
    }
}

/// Standalone message handler to avoid Send/Sync issues with Connection
async fn handle_client_message(
    host_state: Arc<BrioHostState>,
    client_id: ClientId,
    text: &str,
) -> Result<ClientResponse, anyhow::Error> {
    let message: ClientMessage =
        serde_json::from_str(text).map_err(|e| anyhow::anyhow!("Invalid JSON: {e}"))?;

    match message {
        ClientMessage::Task { content } => {
            handle_task_submission(host_state, client_id, content).await
        }
        ClientMessage::Session { action, params } => {
            handle_session_action(host_state, client_id, action, params).await
        }
        ClientMessage::Query { sql } => handle_query(host_state, client_id, sql).await,
    }
}

async fn handle_task_submission(
    host_state: Arc<BrioHostState>,
    _client_id: ClientId,
    content: String,
) -> Result<ClientResponse, anyhow::Error> {
    if content.trim().is_empty() {
        return Ok(ClientResponse::error("Task content cannot be empty"));
    }

    let sql =
        "INSERT INTO tasks (content, priority, status, parent_id) VALUES (?, 10, 'pending', NULL)";
    let result = sqlx::query(sql)
        .bind(&content)
        .execute(host_state.db())
        .await;

    match result {
        Ok(query_result) => {
            let task_id: i64 = query_result.last_insert_rowid();
            info!(task_id, "Task created via WebSocket");
            let data = serde_json::json!({
                "task_id": task_id.to_string(),
                "content": content,
                "status": "pending"
            });
            Ok(ClientResponse::success(Some(data)))
        }
        Err(e) => {
            error!(error = %e, "Failed to create task");
            Ok(ClientResponse::error(format!("Failed to create task: {e}")))
        }
    }
}

async fn handle_session_action(
    host_state: Arc<BrioHostState>,
    _client_id: ClientId,
    action: SessionAction,
    params: crate::ws::types::SessionParams,
) -> Result<ClientResponse, anyhow::Error> {
    match action {
        SessionAction::Begin => {
            let base_path = params
                .base_path
                .ok_or_else(|| anyhow::anyhow!("base_path is required for begin action"))?;

            match host_state.begin_session(&base_path) {
                Ok(session_id) => {
                    let data =
                        serde_json::json!({ "session_id": session_id, "base_path": base_path });
                    Ok(ClientResponse::success(Some(data)))
                }
                Err(e) => Ok(ClientResponse::error(format!(
                    "Failed to begin session: {e}"
                ))),
            }
        }
        SessionAction::Commit => {
            let session_id = params
                .session_id
                .ok_or_else(|| anyhow::anyhow!("session_id is required for commit action"))?;

            match host_state.commit_session(&session_id) {
                Ok(()) => Ok(ClientResponse::success(Some(
                    serde_json::json!({ "session_id": session_id, "action": "committed" }),
                ))),
                Err(e) => Ok(ClientResponse::error(format!(
                    "Failed to commit session: {e}"
                ))),
            }
        }
        SessionAction::Rollback => {
            let session_id = params
                .session_id
                .ok_or_else(|| anyhow::anyhow!("session_id is required for rollback action"))?;

            match host_state.rollback_session(&session_id) {
                Ok(()) => Ok(ClientResponse::success(Some(
                    serde_json::json!({ "session_id": session_id, "action": "rolled_back" }),
                ))),
                Err(e) => Ok(ClientResponse::error(format!(
                    "Failed to rollback session: {e}"
                ))),
            }
        }
    }
}

async fn handle_query(
    host_state: Arc<BrioHostState>,
    _client_id: ClientId,
    sql: String,
) -> Result<ClientResponse, anyhow::Error> {
    if sql.trim().is_empty() {
        return Ok(ClientResponse::error("SQL query cannot be empty"));
    }

    let upper_sql = sql.trim().to_uppercase();
    if upper_sql.starts_with("INSERT")
        || upper_sql.starts_with("UPDATE")
        || upper_sql.starts_with("DELETE")
        || upper_sql.starts_with("DROP")
        || upper_sql.starts_with("CREATE")
        || upper_sql.starts_with("ALTER")
    {
        return Ok(ClientResponse::error(
            "Only SELECT queries are allowed via WebSocket",
        ));
    }

    let result = sqlx::query(&sql).fetch_all(host_state.db()).await;

    match result {
        Ok(rows) => {
            let mut results = Vec::new();
            for row in rows {
                let mut row_map = serde_json::Map::new();
                for (i, col) in row.columns().iter().enumerate() {
                    let value: String = row
                        .try_get::<String, _>(i)
                        .unwrap_or_else(|_| "NULL".to_string());
                    row_map.insert(col.name().to_string(), serde_json::Value::String(value));
                }
                results.push(serde_json::Value::Object(row_map));
            }
            Ok(ClientResponse::success(Some(serde_json::Value::Array(
                results,
            ))))
        }
        Err(e) => {
            error!(error = %e, "Query execution failed");
            Ok(ClientResponse::error(format!("Query failed: {e}")))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn connection_generates_unique_client_id() {
        let id1 = ClientId::generate();
        let id2 = ClientId::generate();
        assert_ne!(id1, id2);
    }
}
