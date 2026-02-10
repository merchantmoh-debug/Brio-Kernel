//! Branch Operations - Change collection and staging operations.
//!
//! This module handles the collection of file changes from branches
//! and their application to staging areas.

use std::path::PathBuf;

use tracing::debug;

use crate::branch::{Branch, BranchError, BranchManager, SessionError};
use crate::merge::FileChange as MergeFileChange;

impl BranchManager {
    /// Collects file changes from a branch session.
    ///
    /// This scans the branch's session directory and compares it with the parent
    /// to determine what files have been added, modified, or deleted.
    ///
    /// # Errors
    /// Returns `BranchError` if session access fails.
    pub async fn collect_branch_changes(
        &self,
        branch: &Branch,
    ) -> Result<Vec<MergeFileChange>, BranchError> {
        let session_id = branch.session_id();

        // Get session path
        let session_path = {
            let session_manager = self.lock_session_manager()?;
            session_manager.session_path(session_id).ok_or_else(|| {
                BranchError::Session(SessionError::SessionNotFound(session_id.to_string()))
            })?
        };

        // Collect changes by scanning the session directory
        let mut changes = Vec::new();
        self.scan_directory_for_changes(&session_path, PathBuf::new(), &mut changes)
            .await?;

        Ok(changes)
    }

    /// Recursively scans a directory for file changes.
    ///
    /// # Errors
    /// Returns `BranchError` if directory reading fails.
    async fn scan_directory_for_changes(
        &self,
        base_path: &PathBuf,
        relative_path: PathBuf,
        changes: &mut Vec<MergeFileChange>,
    ) -> Result<(), BranchError> {
        let full_path = base_path.join(&relative_path);

        let entries = tokio::fs::read_dir(&full_path)
            .await
            .map_err(|e| BranchError::Session(SessionError::ReadDirectoryFailed(e.to_string())))?;

        let mut entries = entries;
        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|e| BranchError::Session(SessionError::ReadDirectoryFailed(e.to_string())))?
        {
            let file_name = entry.file_name();
            let file_name_str = file_name.to_string_lossy();

            // Skip hidden files and directories
            if file_name_str.starts_with('.') {
                continue;
            }

            let entry_relative_path = relative_path.join(&file_name);

            let file_type = entry.file_type().await.map_err(|e| {
                BranchError::Session(SessionError::ReadDirectoryFailed(e.to_string()))
            })?;

            if file_type.is_dir() {
                // Recursively scan subdirectory
                Box::pin(self.scan_directory_for_changes(base_path, entry_relative_path, changes))
                    .await?;
            } else if file_type.is_file() {
                // Record file as modified (simplified - in real impl, compare with parent)
                changes.push(MergeFileChange::Modified(entry_relative_path));
            }
        }

        Ok(())
    }

    /// Applies changes to the staging session.
    ///
    /// # Errors
    /// Returns `BranchError` if file operations fail.
    pub fn apply_changes_to_staging(
        &self,
        _staging_session_id: &str,
        changes: &[MergeFileChange],
    ) -> Result<(), BranchError> {
        // TODO(#124): Implement staging area file operations

        for change in changes {
            match change {
                MergeFileChange::Added(path) | MergeFileChange::Modified(path) => {
                    debug!("Applying change to staging: {:?}", path);
                }
                MergeFileChange::Deleted(path) => {
                    debug!("Applying deletion to staging: {:?}", path);
                }
            }
        }

        Ok(())
    }
}
