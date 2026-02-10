//! Branch Lifecycle - Creation, deletion, and recovery operations.
//!
//! This module handles the lifecycle of branches including creation,
//! abortion/rollback, and recovery after restarts.

use tracing::{info, instrument};

use crate::branch::{BranchError, BranchManager, BranchSource};
use crate::domain::{BranchConfig, BranchId, BranchRecord, BranchStatus, BranchValidationError};

impl BranchManager {
    /// Creates a new branch from the given source.
    ///
    /// # Errors
    ///
    /// Returns `BranchError` if:
    /// - Branch limit is exceeded
    /// - Source branch doesn't exist
    /// - VFS session creation fails
    /// - Repository persistence fails
    #[instrument(skip(self, source, config))]
    pub async fn create_branch(
        &mut self,
        source: BranchSource,
        config: BranchConfig,
    ) -> Result<BranchId, BranchError> {
        // 1. Check branch limit
        self.check_branch_limit()?;

        // 2. Get base path from source
        let base_path = self.get_base_path_from_source(&source)?;
        let base_path_str = base_path.to_string_lossy().to_string();

        // 3. Create VFS session via SessionManager
        let session_id = {
            let mut session_manager = self.lock_session_manager()?;
            session_manager.begin_session(&base_path_str)?
        };

        // 4. Create Branch entity
        let branch_id = BranchManager::next_branch_id();
        let parent_id = match &source {
            BranchSource::Branch(id) => Some(*id),
            _ => None,
        };

        // Serialize config to JSON for storage
        let config_json = serde_json::to_string(&config).map_err(|e| {
            BranchError::Validation(BranchValidationError::InvalidExecutionStrategy {
                reason: format!("Failed to serialize config: {e}"),
            })
        })?;

        let branch = BranchRecord::new(
            branch_id,
            parent_id,
            session_id.clone(),
            config.name().to_string(),
            BranchStatus::Pending,
            chrono::Utc::now().timestamp(),
            None,
            config_json,
        )
        .map_err(|e| {
            BranchError::Validation(BranchValidationError::InvalidExecutionStrategy {
                reason: e.to_string(),
            })
        })?;

        // 5. Persist to repository
        self.repository.create_branch(&branch)?;

        info!(
            "Created branch {} with session {} from source {:?}",
            branch_id, session_id, source
        );

        // 6. Return BranchId
        Ok(branch_id)
    }

    /// Aborts/rolls back a branch.
    ///
    /// # Errors
    ///
    /// Returns `BranchError` if:
    /// - Branch not found
    /// - Session rollback fails
    /// - Repository update fails
    #[instrument(skip(self, id))]
    pub async fn abort_branch(&self, id: BranchId) -> Result<(), BranchError> {
        // 1. Get branch session_id
        let branch = self
            .repository
            .get_branch(id)?
            .ok_or(BranchError::BranchNotFound(id))?;

        let session_id = branch.session_id().to_string();

        // 2. Rollback session via SessionManager
        {
            let mut session_manager = self.lock_session_manager()?;
            session_manager.rollback_session(&session_id)?;
        }

        // 3. Mark branch as Failed
        self.update_status(id, BranchStatus::Failed)?;

        info!(
            "Aborted branch {} and rolled back session {}",
            id, session_id
        );

        Ok(())
    }

    /// Recovers branches after a restart.
    ///
    /// Returns a list of branch IDs that were successfully recovered.
    /// Branches in "Active" status are transitioned back to "Pending".
    ///
    /// # Errors
    ///
    /// Returns `BranchError::Repository` if the query fails.
    #[instrument(skip(self))]
    pub fn recover_branches(&self) -> Result<Vec<BranchId>, BranchError> {
        use tracing::{error, warn};

        // 1. Query all active branches from repository
        let active_branches = self.repository.list_active_branches()?;

        let mut recovered = Vec::new();

        for branch in active_branches {
            let branch_id = branch.id();

            // 2. Validate session still exists
            let session_exists = {
                let session_manager = self.lock_session_manager()?;
                session_manager.session_path(branch.session_id()).is_some()
            };

            if !session_exists {
                warn!(
                    "Branch {} session {} no longer exists, marking as failed",
                    branch_id,
                    branch.session_id()
                );
                let _ = self.update_status(branch_id, BranchStatus::Failed);
                continue;
            }

            // 3. Branches in "Active" should transition back to "Pending"
            if branch.status() == BranchStatus::Active {
                info!("Recovering branch {} from Active to Pending", branch_id);
                if let Err(e) = self
                    .repository
                    .update_branch_status(branch_id, BranchStatus::Pending)
                {
                    error!("Failed to recover branch {}: {}", branch_id, e);
                    continue;
                }
            }

            recovered.push(branch_id);
        }

        info!("Recovered {} branches after restart", recovered.len());

        Ok(recovered)
    }
}
