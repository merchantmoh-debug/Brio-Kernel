//! Execution metrics types for WebSocket broadcasting.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::events::OperationType;

/// Errors that can occur when creating a progress update.
#[derive(Debug, Error)]
pub enum ProgressUpdateError {
    /// Total items must be greater than zero.
    #[error("total_items must be greater than zero")]
    InvalidTotalItems,
    /// Completed items cannot exceed total items.
    #[error("completed_items cannot exceed total_items")]
    InvalidCompletedItems,
}

/// Progress update for long-running operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgressUpdate {
    /// Unique identifier for the operation.
    operation_id: String,
    /// Type of operation.
    operation_type: OperationType,
    /// Total number of items to process.
    total_items: usize,
    /// Number of items completed so far.
    completed_items: usize,
    /// Current item being processed (if any).
    current_item: Option<String>,
    /// Timestamp of the update.
    timestamp: DateTime<Utc>,
}

impl ProgressUpdate {
    /// Creates a new progress update.
    ///
    /// # Arguments
    ///
    /// * `operation_id` - Unique identifier for the operation.
    /// * `operation_type` - Type of operation being performed.
    /// * `total_items` - Total number of items to process (must be > 0).
    /// * `completed_items` - Number of items completed so far.
    /// * `current_item` - Current item being processed (optional).
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// * `total_items` is zero
    /// * `completed_items` exceeds `total_items`
    pub fn new(
        operation_id: String,
        operation_type: OperationType,
        total_items: usize,
        completed_items: usize,
        current_item: Option<String>,
    ) -> Result<Self, ProgressUpdateError> {
        if total_items == 0 {
            return Err(ProgressUpdateError::InvalidTotalItems);
        }
        if completed_items > total_items {
            return Err(ProgressUpdateError::InvalidCompletedItems);
        }
        Ok(Self {
            operation_id,
            operation_type,
            total_items,
            completed_items,
            current_item,
            timestamp: Utc::now(),
        })
    }

    /// Returns the operation ID.
    #[must_use]
    pub fn operation_id(&self) -> &str {
        &self.operation_id
    }

    /// Returns the operation type.
    #[must_use]
    pub fn operation_type(&self) -> OperationType {
        self.operation_type
    }

    /// Returns the total number of items.
    #[must_use]
    pub fn total_items(&self) -> usize {
        self.total_items
    }

    /// Returns the number of completed items.
    #[must_use]
    pub fn completed_items(&self) -> usize {
        self.completed_items
    }

    /// Returns the current item being processed, if any.
    #[must_use]
    pub fn current_item(&self) -> Option<&str> {
        self.current_item.as_deref()
    }

    /// Returns the timestamp of the update.
    #[must_use]
    pub fn timestamp(&self) -> DateTime<Utc> {
        self.timestamp
    }

    /// Calculates the percentage complete (0.0 to 100.0).
    #[must_use]
    pub fn percent_complete(&self) -> f32 {
        (self.completed_items as f32 / self.total_items as f32) * 100.0
    }
}
