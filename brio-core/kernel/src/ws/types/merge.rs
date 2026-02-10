//! Merge request types for WebSocket broadcasting.

use serde::{Deserialize, Serialize};

use super::branch::BranchId;
use super::events::{EventMetadata, MergeStrategy};

/// Merge request events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MergeRequestEvent {
    /// New merge request created
    Created {
        /// Unique identifier for the merge request.
        merge_request_id: String,
        /// ID of the branch to be merged.
        branch_id: BranchId,
        /// Merge strategy proposed.
        strategy: MergeStrategy,
        /// Whether manual approval is required.
        requires_approval: bool,
        /// Common event metadata.
        #[serde(flatten)]
        metadata: EventMetadata,
    },

    /// Merge request approved
    Approved {
        /// ID of the approved merge request.
        merge_request_id: String,
        /// User or system that approved the merge.
        approver: String,
        /// Common event metadata.
        #[serde(flatten)]
        metadata: EventMetadata,
    },

    /// Merge request rejected
    Rejected {
        /// ID of the rejected merge request.
        merge_request_id: String,
        /// Reason for rejection.
        reason: String,
        /// Common event metadata.
        #[serde(flatten)]
        metadata: EventMetadata,
    },

    /// Merge request completed
    Completed {
        /// ID of the completed merge request.
        merge_request_id: String,
        /// ID of the branch that was merged.
        branch_id: BranchId,
        /// Whether the merge was successful.
        success: bool,
        /// Common event metadata.
        #[serde(flatten)]
        metadata: EventMetadata,
    },
}
