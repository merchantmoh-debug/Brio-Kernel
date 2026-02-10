//! Diff computation for VFS session management.
//!
//! This module provides utilities for detecting changes between directories.

use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;
use walkdir::WalkDir;

/// Types of changes detected in a file diff.
#[derive(Debug, Clone)]
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

/// Compute SHA256 hash of a file.
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

/// Scan a directory and collect file metadata.
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

/// Get the relative paths of all changes.
#[must_use]
pub fn get_change_paths(changes: &[FileChange]) -> Vec<&PathBuf> {
    changes
        .iter()
        .map(|c| match c {
            FileChange::Modified(p) | FileChange::Added(p) | FileChange::Deleted(p) => p,
        })
        .collect()
}
