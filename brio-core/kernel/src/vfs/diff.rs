//! File diff computation and application for session management.
//!
//! This module provides utilities for detecting changes between directories
//! and applying them atomically using a staging approach.

use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;
use tracing::{debug, info, warn};
use walkdir::WalkDir;

/// Types of changes detected in a file diff.
#[derive(Debug)]
pub enum FileChange {
    /// File was modified.
    Modified(PathBuf),
    /// File was added.
    Added(PathBuf),
    /// File was deleted.
    Deleted(PathBuf),
}

#[derive(Debug, Clone)]
struct FileMetadata {
    size: u64,
    // This field stores the file modification time for future use in incremental diff operations.
    // It's currently unused but preserved for the complete file metadata API.
    #[expect(dead_code)]
    modified: SystemTime,
    hash: Option<Arc<str>>,
}

fn compute_hash(path: &Path) -> io::Result<Arc<str>> {
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0; 8192];

    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }

    Ok(Arc::from(hex::encode(hasher.finalize())))
}

fn scan_directory(root: &Path) -> io::Result<HashMap<PathBuf, FileMetadata>> {
    let mut map = HashMap::new();

    for entry in WalkDir::new(root) {
        let entry = entry?;
        let path = entry.path();

        if path.is_file()
            && let Ok(relative) = path.strip_prefix(root)
        {
            let metadata = fs::metadata(path)?;
            map.insert(
                relative.to_path_buf(),
                FileMetadata {
                    size: metadata.len(),
                    modified: metadata.modified()?,
                    hash: None,
                },
            );
        }
    }

    Ok(map)
}

/// Computes the difference between a session directory and a base directory.
///
/// # Errors
///
/// Returns an error if directory scanning or file hashing fails.
pub fn compute_diff(session_path: &Path, base_path: &Path) -> io::Result<Vec<FileChange>> {
    let session_files = scan_directory(session_path)?;
    let base_files = scan_directory(base_path)?;
    // Pre-allocate: max possible changes is all session files + all deletions
    let max_changes = session_files.len() + base_files.len();
    let mut changes = Vec::with_capacity(max_changes);

    for (rel_path, session_meta) in &session_files {
        match base_files.get(rel_path) {
            Some(base_meta) => {
                // Short-circuit: If size differs, it IS modified.
                if session_meta.size != base_meta.size {
                    changes.push(FileChange::Modified(rel_path.clone()));
                    continue;
                }

                // If sizes match, verify with hash.
                let s_hash = match &session_meta.hash {
                    Some(h) => h.clone(),
                    None => compute_hash(&session_path.join(rel_path))?,
                };

                let b_hash = match &base_meta.hash {
                    Some(h) => h.clone(),
                    None => compute_hash(&base_path.join(rel_path))?,
                };

                if s_hash != b_hash {
                    changes.push(FileChange::Modified(rel_path.clone()));
                }
            }
            None => {
                changes.push(FileChange::Added(rel_path.clone()));
            }
        }
    }

    // Check for Deleted
    for rel_path in base_files.keys() {
        if !session_files.contains_key(rel_path) {
            changes.push(FileChange::Deleted(rel_path.clone()));
        }
    }

    Ok(changes)
}

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
