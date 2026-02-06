//! WebSocket connection lifecycle management.

use axum::extract::ws::{Message, WebSocket};
use bytes::Bytes;
use futures_util::StreamExt;
use std::time::Duration;
use tokio::time::interval;
use tracing::{debug, error, info, warn};

use crate::ws::broadcaster::BroadcastReceiver;
use crate::ws::types::{BroadcastMessage, ClientId, WsError};

const PING_INTERVAL: Duration = Duration::from_secs(30);

/// A WebSocket connection with a unique client ID.
pub struct Connection {
    client_id: ClientId,
    stream: WebSocket,
    receiver: BroadcastReceiver,
}

impl Connection {
    /// Creates a new WebSocket connection.
    ///
    /// # Arguments
    ///
    /// * `stream` - The WebSocket stream.
    /// * `receiver` - The broadcast receiver for messages.
    pub fn new(stream: WebSocket, receiver: BroadcastReceiver) -> Self {
        let client_id = ClientId::generate();
        info!(client_id = %client_id, "WebSocket connection established");
        Self {
            client_id,
            stream,
            receiver,
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
                warn!(client_id = %self.client_id, "Unexpected text from client");
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
