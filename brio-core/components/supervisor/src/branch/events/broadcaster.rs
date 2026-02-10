//! Branch Event Broadcasting
//!
//! Provides WebSocket event broadcasting for branch lifecycle events.

use std::sync::Arc;

use brio_kernel::ws::broadcaster::Broadcaster;
use brio_kernel::ws::types::WsMessage;

/// Broadcasts branch lifecycle events to WebSocket clients.
#[derive(Clone)]
pub struct BranchEventBroadcaster {
    broadcaster: Arc<Broadcaster>,
}

impl BranchEventBroadcaster {
    /// Creates a new BranchEventBroadcaster.
    #[must_use]
    pub fn new(broadcaster: Arc<Broadcaster>) -> Self {
        Self { broadcaster }
    }

    /// Returns a reference to the underlying broadcaster.
    #[must_use]
    pub fn inner(&self) -> &Arc<Broadcaster> {
        &self.broadcaster
    }

    /// Internal method to broadcast a message.
    pub(crate) fn broadcast(&self,
        message: WsMessage,
    ) {
        use tracing::debug;
        if let Err(e) = self.broadcaster.broadcast_message(message) {
            debug!("Failed to broadcast WebSocket message: {}", e);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use brio_kernel::ws::types::BroadcastMessage;
    use crate::domain::{AgentId, Priority};
    use crate::merge::MergeId;
    use crate::branch::{
        AgentAssignment, BranchConfig, BranchId, BranchResult, ChangeType, ExecutionMetrics,
        ExecutionStrategy, FileChange,
    };
    use std::path::PathBuf;

    fn create_test_broadcaster() -> (Arc<Broadcaster>, BranchEventBroadcaster) {
        let broadcaster = Arc::new(Broadcaster::new());
        let event_broadcaster = BranchEventBroadcaster::new(Arc::clone(&broadcaster));
        (broadcaster, event_broadcaster)
    }

    #[test]
    fn branch_event_broadcaster_creation() {
        let broadcaster = Arc::new(Broadcaster::new());
        let _event_broadcaster = BranchEventBroadcaster::new(broadcaster);
    }

    #[tokio::test]
    async fn broadcast_branch_created_reaches_subscribers() {
        use crate::branch::events::handlers::BranchEventHandlers;
        
        let broadcaster = Arc::new(Broadcaster::new());
        let event_broadcaster = BranchEventBroadcaster::new(Arc::clone(&broadcaster));
        let mut rx = broadcaster.subscribe();

        event_broadcaster.branch_created(
            BranchId::new(),
            None,
            "test-branch",
            "session-123",
        );

        let msg = rx.recv().await.unwrap();
        assert!(matches!(msg, BroadcastMessage::Message(WsMessage::BranchEvent(_))));
    }

    #[tokio::test]
    async fn broadcast_execution_started_reaches_subscribers() {
        use crate::branch::events::handlers::BranchEventHandlers;
        
        let broadcaster = Arc::new(Broadcaster::new());
        let event_broadcaster = BranchEventBroadcaster::new(Arc::clone(&broadcaster));
        let mut rx = broadcaster.subscribe();

        event_broadcaster.execution_started(
            BranchId::new(),
            vec!["agent1".to_string(), "agent2".to_string()],
            "sequential",
        );

        let msg = rx.recv().await.unwrap();
        assert!(matches!(msg, BroadcastMessage::Message(WsMessage::BranchEvent(_))));
    }

    #[tokio::test]
    async fn broadcast_merge_request_created_reaches_subscribers() {
        use crate::branch::events::handlers::MergeEventHandlers;
        
        let broadcaster = Arc::new(Broadcaster::new());
        let event_broadcaster = BranchEventBroadcaster::new(Arc::clone(&broadcaster));
        let mut rx = broadcaster.subscribe();

        event_broadcaster.merge_request_created(
            MergeId::new(),
            BranchId::new(),
            "union",
            true,
        );

        let msg = rx.recv().await.unwrap();
        assert!(matches!(msg, BroadcastMessage::Message(WsMessage::MergeRequestEvent(_))));
    }
}
