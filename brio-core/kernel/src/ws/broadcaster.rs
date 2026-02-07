//! Broadcaster service for JSON Patch distribution.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::sync::broadcast;
use tracing::{debug, warn};

use crate::ws::types::{BroadcastMessage, WsError};

const BROADCAST_CAPACITY: usize = 256;

/// Broadcasts messages to all connected WebSocket clients.
#[derive(Clone)]
pub struct Broadcaster {
    sender: broadcast::Sender<BroadcastMessage>,
    client_count: Arc<AtomicUsize>,
}

impl Broadcaster {
    /// Creates a new broadcaster with an empty channel.
    #[must_use]
    pub fn new() -> Self {
        let (sender, _) = broadcast::channel(BROADCAST_CAPACITY);
        Self {
            sender,
            client_count: Arc::new(AtomicUsize::new(0)),
        }
    }

    /// Subscribes a new client to receive broadcast messages.
    ///
    /// # Returns
    ///
    /// A receiver for the client.
    pub fn subscribe(&self) -> BroadcastReceiver {
        self.client_count.fetch_add(1, Ordering::SeqCst);
        debug!(client_count = self.client_count(), "Client subscribed");
        BroadcastReceiver {
            inner: self.sender.subscribe(),
            client_count: Arc::clone(&self.client_count),
        }
    }

    /// Broadcasts a message to all connected clients.
    ///
    /// # Errors
    ///
    /// Returns an error if the broadcast channel is closed.
    pub fn broadcast(&self, message: BroadcastMessage) -> Result<(), WsError> {
        if let Ok(receiver_count) = self.sender.send(message) {
            debug!(receiver_count, "Broadcast sent");
            Ok(())
        } else {
            warn!("Broadcast sent but no clients connected");
            Ok(())
        }
    }

    /// Returns the number of connected clients.
    #[must_use]
    pub fn client_count(&self) -> usize {
        self.client_count.load(Ordering::SeqCst)
    }

    /// Returns a reference to the broadcast sender.
    #[must_use]
    pub fn sender(&self) -> &broadcast::Sender<BroadcastMessage> {
        &self.sender
    }
}

impl Default for Broadcaster {
    fn default() -> Self {
        Self::new()
    }
}

/// Receiver for broadcast messages from a broadcaster.
pub struct BroadcastReceiver {
    inner: broadcast::Receiver<BroadcastMessage>,
    client_count: Arc<AtomicUsize>,
}

impl BroadcastReceiver {
    /// Receive a broadcast message.
    ///
    /// # Errors
    /// Returns `WsError::ChannelClosed` if the channel is closed or the receiver lagged.
    pub async fn recv(&mut self) -> Result<BroadcastMessage, WsError> {
        self.inner.recv().await.map_err(|e| match e {
            broadcast::error::RecvError::Closed => WsError::ChannelClosed,
            broadcast::error::RecvError::Lagged(count) => {
                warn!(skipped = count, "Receiver lagged");
                WsError::ChannelClosed
            }
        })
    }
}

impl Drop for BroadcastReceiver {
    fn drop(&mut self) {
        self.client_count.fetch_sub(1, Ordering::SeqCst);
        debug!(
            client_count = self.client_count.load(Ordering::SeqCst),
            "Client unsubscribed"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn broadcaster_tracks_client_count() {
        let broadcaster = Broadcaster::new();
        assert_eq!(broadcaster.client_count(), 0);

        let rx1 = broadcaster.subscribe();
        assert_eq!(broadcaster.client_count(), 1);

        let _rx2 = broadcaster.subscribe();
        assert_eq!(broadcaster.client_count(), 2);

        drop(rx1);
        assert_eq!(broadcaster.client_count(), 1);
    }

    #[tokio::test]
    async fn broadcast_reaches_subscribers() -> Result<(), WsError> {
        let broadcaster = Broadcaster::new();
        let mut rx = broadcaster.subscribe();

        broadcaster.broadcast(BroadcastMessage::Shutdown)?;

        let msg = rx.recv().await?;
        assert!(matches!(msg, BroadcastMessage::Shutdown));
        Ok(())
    }

    #[tokio::test]
    async fn broadcast_with_no_subscribers_succeeds() {
        let broadcaster = Broadcaster::new();
        let result = broadcaster.broadcast(BroadcastMessage::Shutdown);
        assert!(result.is_ok());
    }

    #[test]
    fn broadcaster_is_clone() {
        let broadcaster = Broadcaster::new();
        let cloned = broadcaster.clone();
        assert_eq!(broadcaster.client_count(), cloned.client_count());
    }
}
