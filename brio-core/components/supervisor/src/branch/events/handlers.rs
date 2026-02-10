//! Event handler traits for branch lifecycle broadcasting.

use chrono::Utc;

use brio_kernel::ws::types::{
    BranchEvent, BranchId as WsBranchId, ConflictSummary, EventMetadata, ExecutionStrategy,
    FileChangeSummary, MergeRequestEvent, WsMessage,
};

use crate::branch::events::broadcaster::BranchEventBroadcaster;
use crate::branch::{BranchId, BranchResult, ChangeType};
use crate::merge::MergeId;

/// Trait for broadcasting branch lifecycle events.
pub trait BranchEventHandlers {
    /// Broadcasts a branch created event.
    fn branch_created(
        &self,
        branch_id: BranchId,
        parent_id: Option<BranchId>,
        name: &str,
        session_id: &str,
    );

    /// Broadcasts an execution started event.
    fn execution_started(&self, branch_id: BranchId, agents: Vec<String>, execution_strategy: &str);

    /// Broadcasts an execution progress event.
    fn execution_progress(
        &self,
        branch_id: BranchId,
        total_agents: usize,
        completed_agents: usize,
        current_agent: Option<String>,
    );

    /// Broadcasts an agent completed event.
    fn agent_completed(&self, branch_id: BranchId, agent_id: &str, result_summary: &str);

    /// Broadcasts an execution completed event.
    fn execution_completed(&self, branch_id: BranchId, result: &BranchResult);

    /// Broadcasts an execution failed event.
    fn execution_failed(&self, branch_id: BranchId, error: &str, failed_agent: Option<String>);

    /// Broadcasts a rolled back event.
    fn rolled_back(&self, branch_id: BranchId, reason: &str);
}

/// Trait for broadcasting merge events.
pub trait MergeEventHandlers {
    /// Broadcasts a merge started event.
    fn merge_started(&self, branch_id: BranchId, strategy: &str, requires_approval: bool);

    /// Broadcasts a merge completed event.
    fn merge_completed(&self, branch_id: BranchId, strategy_used: &str, files_changed: usize);

    /// Broadcasts a merge conflict event.
    fn merge_conflict(
        &self,
        branch_id: BranchId,
        conflicts: Vec<ConflictSummary>,
        merge_request_id: MergeId,
    );

    /// Broadcasts a merge request created event.
    fn merge_request_created(
        &self,
        merge_request_id: MergeId,
        branch_id: BranchId,
        strategy: &str,
        requires_approval: bool,
    );

    /// Broadcasts a merge request approved event.
    fn merge_request_approved(&self, merge_request_id: MergeId, approver: &str);

    /// Broadcasts a merge request rejected event.
    fn merge_request_rejected(&self, merge_request_id: MergeId, reason: &str);

    /// Broadcasts a merge request completed event.
    fn merge_request_completed(
        &self,
        merge_request_id: MergeId,
        branch_id: BranchId,
        success: bool,
    );
}

impl BranchEventHandlers for BranchEventBroadcaster {
    fn branch_created(
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

    fn execution_started(
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

    fn execution_progress(
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

    fn agent_completed(&self, branch_id: BranchId, agent_id: &str, result_summary: &str) {
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

    fn execution_completed(&self, branch_id: BranchId, result: &BranchResult) {
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

    fn execution_failed(&self, branch_id: BranchId, error: &str, failed_agent: Option<String>) {
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

    fn rolled_back(&self, branch_id: BranchId, reason: &str) {
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
}

impl MergeEventHandlers for BranchEventBroadcaster {
    fn merge_started(&self, branch_id: BranchId, strategy: &str, requires_approval: bool) {
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

    fn merge_completed(&self, branch_id: BranchId, strategy_used: &str, files_changed: usize) {
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

    fn merge_conflict(
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

    fn merge_request_created(
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

    fn merge_request_approved(&self, merge_request_id: MergeId, approver: &str) {
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

    fn merge_request_rejected(&self, merge_request_id: MergeId, reason: &str) {
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

    fn merge_request_completed(
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
}
