//! Diff application for VFS session management.
//!
//! This module provides utilities for applying file changes atomically
/// using a staging approach.
use std::fs;
use std::io;
use std::path::Path;
use tracing::{debug, info, warn};

use super::compute::FileChange;

/// Applies file changes from a session directory to a base directory.
///
/// # Errors
///
/// Returns an error if file operations fail during the apply process.
pub fn apply_changes(
    session_path: &Path,
    base_path: &Path,
    changes: &[FileChange],
) -> io::Result<()> {
    if changes.is_empty() {
        return Ok(());
    }

    let staging_dir_name = format!(".commit_{}", uuid::Uuid::new_v4());
    let staging_path = base_path.join(&staging_dir_name);

    if !staging_path.exists() {
        fs::create_dir_all(&staging_path)?;
    }

    // Phase 1: Prepare - Stage content
    for change in changes {
        match change {
            FileChange::Added(rel) | FileChange::Modified(rel) => {
                let src = session_path.join(rel);
                let dst = staging_path.join(rel);

                if let Some(parent) = dst.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::copy(&src, &dst)?;
            }
            FileChange::Deleted(_) => {}
        }
    }

    debug!("Phase 1 Prepare complete. Staging at {:?}", staging_path);

    // Phase 2: Finalize
    // We wrap this in a closure (or block) to ensure cleanup executes.
    // Order: Deletions, then Moves (Staging -> Final).

    let finalize_result = || -> io::Result<()> {
        // Step 2a: Deletions
        for change in changes {
            if let FileChange::Deleted(rel) = change {
                let target = base_path.join(rel);
                if target.exists() {
                    fs::remove_file(&target)?;
                }
            }
        }

        // Step 2b: Moves (Staging -> Final)
        for change in changes {
            if let FileChange::Added(rel) | FileChange::Modified(rel) = change {
                let staged_file = staging_path.join(rel);
                let final_dest = base_path.join(rel);

                if let Some(parent) = final_dest.parent() {
                    fs::create_dir_all(parent)?;
                }

                // If final destination is a directory (and we are replacing it with a file),
                // we must remove the directory first.
                if final_dest.is_dir() {
                    debug!("Removing conflicting directory at {:?}", final_dest);
                    fs::remove_dir_all(&final_dest)?;
                }

                if let Err(e) = fs::rename(&staged_file, &final_dest) {
                    warn!(
                        "Failed to rename staged file {:?} to {:?}: {}",
                        staged_file, final_dest, e
                    );
                    return Err(e);
                }
            }
        }
        Ok(())
    }();

    // Cleanup staging directory
    if staging_path.exists() {
        let _ = fs::remove_dir_all(&staging_path);
    }

    finalize_result?;

    info!(
        "Applied {} changes to Base via Atomic Staging",
        changes.len()
    );
    Ok(())
}

/// Apply a single change (for testing or selective application).
///
/// # Errors
///
/// Returns an error if the file operation fails.
pub fn apply_single_change(
    session_path: &Path,
    base_path: &Path,
    change: &FileChange,
) -> io::Result<()> {
    match change {
        FileChange::Added(rel) | FileChange::Modified(rel) => {
            let src = session_path.join(rel);
            let dst = base_path.join(rel);

            if let Some(parent) = dst.parent() {
                fs::create_dir_all(parent)?;
            }

            // If destination is a directory, remove it first
            if dst.is_dir() {
                fs::remove_dir_all(&dst)?;
            }

            fs::copy(&src, &dst)?;
            Ok(())
        }
        FileChange::Deleted(rel) => {
            let target = base_path.join(rel);
            if target.exists() {
                if target.is_dir() {
                    fs::remove_dir_all(&target)?;
                } else {
                    fs::remove_file(&target)?;
                }
            }
            Ok(())
        }
    }
}
