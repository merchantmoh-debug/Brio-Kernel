//! Branch Event Broadcasting
//!
//! Provides WebSocket event broadcasting for branch lifecycle events.

use std::sync::Arc;

use chrono::Utc;
use tracing::debug;

use brio_kernel::ws::broadcaster::Broadcaster;
use brio_kernel::ws::types::{
    BranchEvent, BranchId as WsBranchId, ConflictSummary, EventMetadata, ExecutionStrategy,
    FileChangeSummary, MergeRequestEvent, WsMessage,
};

use crate::branch::{BranchId, BranchResult, ChangeType};
use crate::merge::MergeId;

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

    /// Broadcasts a branch created event.
    pub fn branch_created(
        &self,
        branch_id: BranchId,
        parent_id: Option<BranchId>,
        name: &str,
        session_id: &str,
    ) {
        let event = BranchEvent::Created {
            branch_id: WsBranchId::new(branch_id.to_string()),
            parent_id: parent_id.map(|id| WsBranchId::new(id.to_string())),
            name: name.to_string(),
            session_id: session_id.to_string(),
            metadata: EventMetadata {
                event_id: format!("evt_{}", Utc::now().timestamp_millis()),
                timestamp: Utc::now(),
            },
        };
        self.broadcast(WsMessage::BranchEvent(event));
    }

    /// Broadcasts an execution started event.
    pub fn execution_started(
        &self,
        branch_id: BranchId,
        agents: Vec<String>,
        execution_strategy: &str,
    ) {
        let strategy = match execution_strategy {
            "parallel" => ExecutionStrategy::Parallel,
            _ => ExecutionStrategy::Sequential,
        };
        let event = BranchEvent::ExecutionStarted {
            branch_id: WsBranchId::new(branch_id.to_string()),
            agents,
            execution_strategy: strategy,
            metadata: EventMetadata {
                event_id: format!("evt_{}", Utc::now().timestamp_millis()),
                timestamp: Utc::now(),
            },
        };
        self.broadcast(WsMessage::BranchEvent(event));
    }

    /// Broadcasts an execution progress event.
    pub fn execution_progress(
        &self,
        branch_id: BranchId,
        total_agents: usize,
        completed_agents: usize,
        current_agent: Option<String>,
    ) {
        let event = BranchEvent::ExecutionProgress {
            branch_id: WsBranchId::new(branch_id.to_string()),
            total_agents,
            completed_agents,
            current_agent,
            metadata: EventMetadata {
                event_id: format!("evt_{}", Utc::now().timestamp_millis()),
                timestamp: Utc::now(),
            },
        };
        self.broadcast(WsMessage::BranchEvent(event));
    }

    /// Broadcasts an agent completed event.
    pub fn agent_completed(&self, branch_id: BranchId, agent_id: &str, result_summary: &str) {
        let event = BranchEvent::AgentCompleted {
            branch_id: WsBranchId::new(branch_id.to_string()),
            agent_id: agent_id.to_string(),
            result_summary: result_summary.to_string(),
            metadata: EventMetadata {
                event_id: format!("evt_{}", Utc::now().timestamp_millis()),
                timestamp: Utc::now(),
            },
        };
        self.broadcast(WsMessage::BranchEvent(event));
    }

    /// Broadcasts an execution completed event.
    pub fn execution_completed(&self, branch_id: BranchId, result: &BranchResult) {
        let file_changes = result
            .file_changes()
            .iter()
            .map(|fc| {
                let change_type = match fc.change_type() {
                    ChangeType::Added => brio_kernel::ws::types::ChangeType::Added,
                    ChangeType::Modified => brio_kernel::ws::types::ChangeType::Modified,
                    ChangeType::Deleted => brio_kernel::ws::types::ChangeType::Deleted,
                    ChangeType::Renamed => brio_kernel::ws::types::ChangeType::Modified, // Map Renamed to Modified for WS
                };
                FileChangeSummary::new(
                    fc.path().to_string_lossy().to_string(),
                    change_type,
                    None, // Would need to compute from diff
                )
            })
            .collect();

        let result_summary = brio_kernel::ws::types::BranchResultSummary::new(
            file_changes,
            result.agent_results().len(),
            result.metrics().total_duration_ms / 1000,
        );

        let event = BranchEvent::ExecutionCompleted {
            branch_id: WsBranchId::new(branch_id.to_string()),
            result: result_summary,
            file_changes_count: result.file_changes().len(),
            metadata: EventMetadata {
                event_id: format!("evt_{}", Utc::now().timestamp_millis()),
                timestamp: Utc::now(),
            },
        };
        self.broadcast(WsMessage::BranchEvent(event));
    }

    /// Broadcasts an execution failed event.
    pub fn execution_failed(
        &self,
        branch_id: BranchId,
        error: &str,
        failed_agent: Option<String>,
    ) {
        let event = BranchEvent::ExecutionFailed {
            branch_id: WsBranchId::new(branch_id.to_string()),
            error: error.to_string(),
            failed_agent,
            metadata: EventMetadata {
                event_id: format!("evt_{}", Utc::now().timestamp_millis()),
                timestamp: Utc::now(),
            },
        };
        self.broadcast(WsMessage::BranchEvent(event));
    }

    /// Broadcasts a merge started event.
    pub fn merge_started(&self, branch_id: BranchId, strategy: &str, requires_approval: bool) {
        use brio_kernel::ws::types::MergeStrategy;
        let strategy_enum = match strategy {
            "fast_forward" => MergeStrategy::FastForward,
            "merge_commit" => MergeStrategy::MergeCommit,
            "squash" => MergeStrategy::Squash,
            "rebase" => MergeStrategy::Rebase,
            _ => MergeStrategy::MergeCommit,
        };
        let event = BranchEvent::MergeStarted {
            branch_id: WsBranchId::new(branch_id.to_string()),
            strategy: strategy_enum,
            requires_approval,
            metadata: EventMetadata {
                event_id: format!("evt_{}", Utc::now().timestamp_millis()),
                timestamp: Utc::now(),
            },
        };
        self.broadcast(WsMessage::BranchEvent(event));
    }

    /// Broadcasts a merge completed event.
    pub fn merge_completed(&self, branch_id: BranchId, strategy_used: &str, files_changed: usize) {
        use brio_kernel::ws::types::MergeStrategy;
        let strategy_enum = match strategy_used {
            "fast_forward" => MergeStrategy::FastForward,
            "merge_commit" => MergeStrategy::MergeCommit,
            "squash" => MergeStrategy::Squash,
            "rebase" => MergeStrategy::Rebase,
            _ => MergeStrategy::MergeCommit,
        };
        let event = BranchEvent::MergeCompleted {
            branch_id: WsBranchId::new(branch_id.to_string()),
            strategy_used: strategy_enum,
            files_changed,
            metadata: EventMetadata {
                event_id: format!("evt_{}", Utc::now().timestamp_millis()),
                timestamp: Utc::now(),
            },
        };
        self.broadcast(WsMessage::BranchEvent(event));
    }

    /// Broadcasts a merge conflict event.
    pub fn merge_conflict(
        &self,
        branch_id: BranchId,
        conflicts: Vec<ConflictSummary>,
        merge_request_id: MergeId,
    ) {
        let event = BranchEvent::MergeConflict {
            branch_id: WsBranchId::new(branch_id.to_string()),
            conflicts,
            merge_request_id: merge_request_id.to_string(),
            metadata: EventMetadata {
                event_id: format!("evt_{}", Utc::now().timestamp_millis()),
                timestamp: Utc::now(),
            },
        };
        self.broadcast(WsMessage::BranchEvent(event));
    }

    /// Broadcasts a rolled back event.
    pub fn rolled_back(&self, branch_id: BranchId, reason: &str) {
        let event = BranchEvent::RolledBack {
            branch_id: WsBranchId::new(branch_id.to_string()),
            reason: reason.to_string(),
            metadata: EventMetadata {
                event_id: format!("evt_{}", Utc::now().timestamp_millis()),
                timestamp: Utc::now(),
            },
        };
        self.broadcast(WsMessage::BranchEvent(event));
    }

    /// Broadcasts a merge request created event.
    pub fn merge_request_created(
        &self,
        merge_request_id: MergeId,
        branch_id: BranchId,
        strategy: &str,
        requires_approval: bool,
    ) {
        use brio_kernel::ws::types::MergeStrategy;
        let strategy_enum = match strategy {
            "fast_forward" => MergeStrategy::FastForward,
            "merge_commit" => MergeStrategy::MergeCommit,
            "squash" => MergeStrategy::Squash,
            "rebase" => MergeStrategy::Rebase,
            _ => MergeStrategy::MergeCommit,
        };
        let event = MergeRequestEvent::Created {
            merge_request_id: merge_request_id.to_string(),
            branch_id: WsBranchId::new(branch_id.to_string()),
            strategy: strategy_enum,
            requires_approval,
            metadata: EventMetadata {
                event_id: format!("evt_{}", Utc::now().timestamp_millis()),
                timestamp: Utc::now(),
            },
        };
        self.broadcast(WsMessage::MergeRequestEvent(event));
    }

    /// Broadcasts a merge request approved event.
    pub fn merge_request_approved(&self, merge_request_id: MergeId, approver: &str) {
        let event = MergeRequestEvent::Approved {
            merge_request_id: merge_request_id.to_string(),
            approver: approver.to_string(),
            metadata: EventMetadata {
                event_id: format!("evt_{}", Utc::now().timestamp_millis()),
                timestamp: Utc::now(),
            },
        };
        self.broadcast(WsMessage::MergeRequestEvent(event));
    }

    /// Broadcasts a merge request rejected event.
    pub fn merge_request_rejected(&self, merge_request_id: MergeId, reason: &str) {
        let event = MergeRequestEvent::Rejected {
            merge_request_id: merge_request_id.to_string(),
            reason: reason.to_string(),
            metadata: EventMetadata {
                event_id: format!("evt_{}", Utc::now().timestamp_millis()),
                timestamp: Utc::now(),
            },
        };
        self.broadcast(WsMessage::MergeRequestEvent(event));
    }

    /// Broadcasts a merge request completed event.
    pub fn merge_request_completed(
        &self,
        merge_request_id: MergeId,
        branch_id: BranchId,
        success: bool,
    ) {
        let event = MergeRequestEvent::Completed {
            merge_request_id: merge_request_id.to_string(),
            branch_id: WsBranchId::new(branch_id.to_string()),
            success,
            metadata: EventMetadata {
                event_id: format!("evt_{}", Utc::now().timestamp_millis()),
                timestamp: Utc::now(),
            },
        };
        self.broadcast(WsMessage::MergeRequestEvent(event));
    }

    /// Internal method to broadcast a message.
    fn broadcast(&self, message: WsMessage) {
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

    #[test]
    fn branch_event_serialization() {
        let event = BranchEvent::Created {
            branch_id: WsBranchId::new("branch_123".to_string()),
            parent_id: Some(WsBranchId::new("branch_456".to_string())),
            name: "test-branch".to_string(),
            session_id: "session-789".to_string(),
            metadata: EventMetadata {
                event_id: "evt_1234567890".to_string(),
                timestamp: Utc::now(),
            },
        };

        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("branch_123"));
        assert!(json.contains("test-branch"));
    }

    #[test]
    fn merge_request_event_serialization() {
        let event = MergeRequestEvent::Approved {
            merge_request_id: "merge_123".to_string(),
            approver: "user_1".to_string(),
            metadata: EventMetadata {
                event_id: "evt_1234567890".to_string(),
                timestamp: Utc::now(),
            },
        };

        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("merge_123"));
        assert!(json.contains("user_1"));
    }

    #[test]
    fn ws_message_roundtrip() {
        let event = BranchEvent::ExecutionFailed {
            branch_id: WsBranchId::new("branch_123".to_string()),
            error: "test error".to_string(),
            failed_agent: Some("agent_1".to_string()),
            metadata: EventMetadata {
                event_id: "evt_1234567890".to_string(),
                timestamp: Utc::now(),
            },
        };

        let message = WsMessage::BranchEvent(event);
        let json = serde_json::to_string(&message).unwrap();
        let deserialized: WsMessage = serde_json::from_str(&json).unwrap();

        match deserialized {
            WsMessage::BranchEvent(BranchEvent::ExecutionFailed { branch_id, error, .. }) => {
                assert_eq!(branch_id.as_str(), "branch_123");
                assert_eq!(error, "test error");
            }
            _ => panic!("Deserialized message doesn't match"),
        }
    }
}
