//! Merge Conflicts - Conflict detection and resolution types.
//!
//! This module defines the core types for merge conflicts including
//! the Conflict struct, `MergeResult`, and error types.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};
use thiserror::Error;

use crate::domain::BranchId;

/// Unique identifier for a merge operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MergeId(uuid::Uuid);

impl MergeId {
    /// Creates a new unique merge ID.
    #[must_use]
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4())
    }

    /// Creates a `MergeId` from an existing UUID.
    #[must_use]
    pub const fn from_uuid(uuid: uuid::Uuid) -> Self {
        Self(uuid)
    }

    /// Returns the inner UUID.
    #[must_use]
    pub const fn inner(&self) -> uuid::Uuid {
        self.0
    }
}

impl Default for MergeId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for MergeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Represents a change to a file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileChange {
    /// File was modified.
    Modified(PathBuf),
    /// File was added.
    Added(PathBuf),
    /// File was deleted.
    Deleted(PathBuf),
}

impl FileChange {
    /// Path of the file that was changed.
    #[must_use]
    pub fn path(&self) -> &Path {
        match self {
            Self::Modified(p) | Self::Added(p) | Self::Deleted(p) => p,
        }
    }

    /// Whether this change represents a file deletion.
    #[must_use]
    pub const fn is_deletion(&self) -> bool {
        matches!(self, Self::Deleted(_))
    }

    /// Whether this change represents a file addition.
    #[must_use]
    pub const fn is_addition(&self) -> bool {
        matches!(self, Self::Added(_))
    }

    /// Whether this change represents a file modification.
    #[must_use]
    pub const fn is_modification(&self) -> bool {
        matches!(self, Self::Modified(_))
    }
}

/// Represents the result of a branch operation with detected changes.
#[derive(Debug, Clone)]
pub struct BranchResult {
    /// The unique identifier for this branch.
    pub branch_id: BranchId,
    /// Path to the branch's working directory.
    pub path: PathBuf,
    /// Changes detected in this branch relative to base.
    pub changes: Vec<FileChange>,
}

impl BranchResult {
    /// Creates a new branch result.
    #[must_use]
    pub fn new(branch_id: BranchId, path: PathBuf, changes: Vec<FileChange>) -> Self {
        Self {
            branch_id,
            path,
            changes,
        }
    }
}

/// Represents a conflict between changes from different branches.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Conflict {
    path: PathBuf,
    branch_ids: Vec<BranchId>,
    description: String,
    line_start: usize,
    line_end: usize,
    base_content: String,
    branch_a_content: String,
    branch_b_content: String,
}

impl Conflict {
    /// Creates a new file-level conflict.
    #[must_use]
    pub fn new(path: PathBuf, branch_ids: Vec<BranchId>, description: impl Into<String>) -> Self {
        Self {
            path,
            branch_ids,
            description: description.into(),
            line_start: 0,
            line_end: 0,
            base_content: String::new(),
            branch_a_content: String::new(),
            branch_b_content: String::new(),
        }
    }

    /// Creates a new line-level conflict with detailed information.
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn with_line_info(
        path: PathBuf,
        branch_ids: Vec<BranchId>,
        description: impl Into<String>,
        line_start: usize,
        line_end: usize,
        base_content: impl Into<String>,
        content_a: impl Into<String>,
        content_b: impl Into<String>,
    ) -> Self {
        Self {
            path,
            branch_ids,
            description: description.into(),
            line_start,
            line_end,
            base_content: base_content.into(),
            branch_a_content: content_a.into(),
            branch_b_content: content_b.into(),
        }
    }

    /// File path where the conflict occurred.
    #[must_use]
    pub fn path(&self) -> &PathBuf {
        &self.path
    }

    /// Branch IDs involved in the conflict.
    #[must_use]
    pub fn branch_ids(&self) -> &[BranchId] {
        &self.branch_ids
    }

    /// Human-readable description of the conflict.
    #[must_use]
    pub fn description(&self) -> &str {
        &self.description
    }

    /// Start line number of the conflict (1-based, 0 for file-level).
    #[must_use]
    pub const fn line_start(&self) -> usize {
        self.line_start
    }

    /// End line number of the conflict (1-based, 0 for file-level).
    #[must_use]
    pub const fn line_end(&self) -> usize {
        self.line_end
    }

    /// Base version content (empty for file-level conflicts).
    #[must_use]
    pub fn base_content(&self) -> &str {
        &self.base_content
    }

    /// Content from the first branch (empty for file-level conflicts).
    #[must_use]
    pub fn branch_a_content(&self) -> &str {
        &self.branch_a_content
    }

    /// Content from the second branch (empty for file-level conflicts).
    #[must_use]
    pub fn branch_b_content(&self) -> &str {
        &self.branch_b_content
    }

    /// Whether this is a line-level conflict with detailed information.
    #[must_use]
    pub const fn has_line_info(&self) -> bool {
        self.line_start > 0
    }

    /// Formats the conflict using Git-style conflict markers.
    ///
    /// This is useful for displaying conflicts to users or writing them to files.
    #[must_use]
    pub fn format_with_markers(&self, left_branch: &str, right_branch: &str) -> String {
        if !self.has_line_info() {
            return format!("File-level conflict at {}", self.path().display());
        }

        let mut output = String::new();

        // Opening marker
        output.push_str("<<<<<<< ");
        output.push_str(left_branch);
        output.push('\n');
        output.push_str(self.branch_a_content());
        if !self.branch_a_content().ends_with('\n') && !self.branch_a_content().is_empty() {
            output.push('\n');
        }

        // Base content marker (if available)
        if !self.base_content().is_empty() {
            output.push_str("||||||| base\n");
            output.push_str(self.base_content());
            if !self.base_content().ends_with('\n') {
                output.push('\n');
            }
        }

        // Separator
        output.push_str("=======\n");

        // Branch B content
        output.push_str(self.branch_b_content());
        if !self.branch_b_content().ends_with('\n') && !self.branch_b_content().is_empty() {
            output.push('\n');
        }

        // Closing marker
        output.push_str(">>>>>>> ");
        output.push_str(right_branch);
        output.push('\n');

        output
    }
}

/// The result of a merge operation.
#[derive(Debug, Clone)]
pub struct MergeResult {
    /// The merged changes that can be applied without conflicts.
    pub merged_changes: Vec<FileChange>,
    /// Conflicts that need resolution.
    pub conflicts: Vec<Conflict>,
    /// The strategy used for this merge.
    pub strategy_used: String,
}

impl MergeResult {
    /// Creates a new successful merge result with no conflicts.
    #[must_use]
    pub fn success(changes: Vec<FileChange>, strategy: impl Into<String>) -> Self {
        Self {
            merged_changes: changes,
            conflicts: Vec::new(),
            strategy_used: strategy.into(),
        }
    }

    /// Creates a new merge result with conflicts.
    #[must_use]
    pub fn with_conflicts(
        changes: Vec<FileChange>,
        conflicts: Vec<Conflict>,
        strategy: impl Into<String>,
    ) -> Self {
        Self {
            merged_changes: changes,
            conflicts,
            strategy_used: strategy.into(),
        }
    }

    /// Returns true if the merge has unresolved conflicts.
    #[must_use]
    pub const fn has_conflicts(&self) -> bool {
        !self.conflicts.is_empty()
    }
}

/// Errors that can occur during diff operations.
#[derive(Debug, Error)]
pub enum DiffError {
    /// I/O error during file operations.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    /// Failed to compute file hash.
    #[error("Failed to compute hash for {path}: {source}")]
    HashComputation {
        /// Path to the file.
        path: PathBuf,
        /// Source error.
        #[source]
        source: std::io::Error,
    },
    /// Path is not valid UTF-8.
    #[error("Path is not valid UTF-8: {0}")]
    InvalidPath(PathBuf),
}

/// Errors that can occur during merge operations.
#[derive(Debug, Error)]
pub enum MergeError {
    /// I/O error during merge.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    /// Diff computation failed.
    #[error("Diff error: {0}")]
    Diff(#[from] DiffError),
    /// Unresolved conflicts remain.
    #[error("Unresolved conflicts: {count} conflicts remain")]
    ConflictsUnresolved {
        /// The conflicts that remain unresolved.
        conflicts: Vec<Conflict>,
        /// The count of unresolved conflicts.
        count: usize,
    },
    /// Branch not found.
    #[error("Branch not found: {0}")]
    BranchNotFound(BranchId),
    /// Invalid strategy specified.
    #[error("Invalid merge strategy: {0}")]
    InvalidStrategy(String),
    /// Maximum number of branches exceeded.
    #[error("Too many branches: got {0}, maximum is 8")]
    TooManyBranches(usize),
}

/// Maximum number of branches allowed in a merge operation.
pub const MAX_BRANCHES: usize = 8;

/// Checks if a file is binary by examining its content.
///
/// # Errors
///
/// Returns an error if the file cannot be read.
pub fn is_binary_file(path: &Path) -> Result<bool, std::io::Error> {
    use std::fs::File;
    use std::io::Read;

    const SAMPLE_SIZE: usize = 8192;

    let mut file = File::open(path)?;
    let mut buffer = vec![0u8; SAMPLE_SIZE];
    let bytes_read = file.read(&mut buffer)?;
    buffer.truncate(bytes_read);

    // Check for null bytes (common in binary files)
    if buffer.contains(&0) {
        return Ok(true);
    }

    // Check for high ratio of non-printable characters
    let non_printable = buffer
        .iter()
        .filter(|&&b| b != b'\n' && b != b'\r' && b != b'\t' && !(32..=126).contains(&b))
        .count();

    let ratio = if bytes_read > 0 {
        #[allow(clippy::cast_precision_loss)]
        let result = non_printable as f64 / bytes_read as f64;
        result
    } else {
        0.0
    };

    // If more than 30% non-printable, consider it binary
    Ok(ratio > 0.3)
}

/// Detects conflicts between multiple branch results.
///
/// # Errors
///
/// Returns an error if conflict detection fails due to I/O or diff errors.
pub fn detect_conflicts(
    _base_path: &Path,
    branches: &[BranchResult],
) -> Result<Vec<Conflict>, DiffError> {
    let mut conflicts = Vec::new();
    let mut file_to_branches: HashMap<PathBuf, Vec<BranchId>> = HashMap::new();

    // Map each file to the branches that changed it
    for branch in branches {
        for change in &branch.changes {
            let path = change.path().to_path_buf();
            file_to_branches
                .entry(path)
                .or_default()
                .push(branch.branch_id);
        }
    }

    // Find files changed by multiple branches
    for (path, branch_ids) in file_to_branches {
        if branch_ids.len() > 1 {
            // Check if changes actually conflict
            let changes: Vec<&FileChange> = branches
                .iter()
                .flat_map(|b| b.changes.iter().filter(|c| c.path() == path))
                .collect();

            let mut has_conflict = false;
            for i in 0..changes.len() {
                for j in (i + 1)..changes.len() {
                    if changes_conflict(changes[i], changes[j]) {
                        has_conflict = true;
                        break;
                    }
                }
                if has_conflict {
                    break;
                }
            }

            if has_conflict {
                conflicts.push(Conflict::new(
                    path.clone(),
                    branch_ids,
                    format!(
                        "Multiple branches have conflicting changes for {}",
                        path.display()
                    ),
                ));
            }
        }
    }

    Ok(conflicts)
}

/// Checks if two file changes conflict.
///
/// Changes conflict if:
/// - They are the same file and both are modifications
/// - One is a deletion and the other is a modification/addition
/// - Both are additions (can't add the same file twice with different content)
#[must_use]
pub fn changes_conflict(change1: &FileChange, change2: &FileChange) -> bool {
    // Different paths never conflict
    if change1.path() != change2.path() {
        return false;
    }

    matches!(
        (change1, change2),
        (FileChange::Modified(_), FileChange::Modified(_))
            | (FileChange::Deleted(_), _)
            | (_, FileChange::Deleted(_))
            | (FileChange::Added(_), FileChange::Added(_))
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_branch_result(id: BranchId, changes: Vec<FileChange>) -> BranchResult {
        BranchResult {
            branch_id: id,
            path: PathBuf::from("/tmp/test"),
            changes,
        }
    }

    #[test]
    fn test_merge_id_creation() {
        let id1 = MergeId::new();
        let id2 = MergeId::new();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_file_change_path_extraction() {
        let path = PathBuf::from("src/main.rs");
        assert_eq!(FileChange::Modified(path.clone()).path(), path.as_path());
        assert_eq!(FileChange::Added(path.clone()).path(), path.as_path());
        assert_eq!(FileChange::Deleted(path.clone()).path(), path.as_path());
    }

    #[test]
    fn test_file_change_type_checks() {
        let path = PathBuf::from("test.txt");
        assert!(FileChange::Added(path.clone()).is_addition());
        assert!(!FileChange::Added(path.clone()).is_modification());
        assert!(!FileChange::Added(path.clone()).is_deletion());

        assert!(FileChange::Modified(path.clone()).is_modification());
        assert!(FileChange::Deleted(path.clone()).is_deletion());
    }

    #[test]
    fn test_merge_result_success() {
        let changes = vec![FileChange::Added(PathBuf::from("file.txt"))];
        let result = MergeResult::success(changes.clone(), "test");

        assert!(!result.has_conflicts());
        assert_eq!(result.merged_changes.len(), 1);
        assert_eq!(result.strategy_used, "test");
    }

    #[test]
    fn test_merge_result_with_conflicts() {
        let changes = vec![FileChange::Added(PathBuf::from("file1.txt"))];
        let conflicts = vec![Conflict::new(
            PathBuf::from("file2.txt"),
            vec![BranchId::new()],
            "test conflict",
        )];
        let result = MergeResult::with_conflicts(changes, conflicts.clone(), "test");

        assert!(result.has_conflicts());
        assert_eq!(result.conflicts.len(), 1);
    }

    #[test]
    fn test_changes_conflict_same_file_modifications() {
        let path = PathBuf::from("test.txt");
        let change1 = FileChange::Modified(path.clone());
        let change2 = FileChange::Modified(path.clone());

        assert!(changes_conflict(&change1, &change2));
    }

    #[test]
    fn test_changes_conflict_different_files() {
        let change1 = FileChange::Modified(PathBuf::from("file1.txt"));
        let change2 = FileChange::Modified(PathBuf::from("file2.txt"));

        assert!(!changes_conflict(&change1, &change2));
    }

    #[test]
    fn test_changes_conflict_deletion() {
        let path = PathBuf::from("test.txt");
        let deletion = FileChange::Deleted(path.clone());
        let modification = FileChange::Modified(path.clone());
        let addition = FileChange::Added(path.clone());

        assert!(changes_conflict(&deletion, &modification));
        assert!(changes_conflict(&deletion, &addition));
        assert!(changes_conflict(&modification, &deletion));
    }

    #[test]
    fn test_changes_conflict_additions() {
        let path = PathBuf::from("test.txt");
        let add1 = FileChange::Added(path.clone());
        let add2 = FileChange::Added(path.clone());

        assert!(changes_conflict(&add1, &add2));
    }

    #[test]
    fn test_conflict_creation() {
        let path = PathBuf::from("test.txt");
        let branch_ids = vec![BranchId::new(), BranchId::new()];
        let conflict = Conflict::new(path.clone(), branch_ids.clone(), "test description");

        assert_eq!(conflict.path(), &path);
        assert_eq!(conflict.branch_ids().len(), 2);
        assert_eq!(conflict.description(), "test description");
    }

    #[test]
    fn test_is_binary_file_text() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        fs::write(&file_path, "Hello, World!\nThis is a text file.").unwrap();

        assert!(!is_binary_file(&file_path).unwrap());
    }

    #[test]
    fn test_is_binary_file_binary() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.bin");
        fs::write(&file_path, vec![0u8, 1, 2, 0, 3, 4]).unwrap();

        assert!(is_binary_file(&file_path).unwrap());
    }

    #[test]
    fn test_detect_conflicts_no_conflict() {
        let branch1 = create_test_branch_result(
            BranchId::new(),
            vec![FileChange::Added(PathBuf::from("file1.txt"))],
        );
        let branch2 = create_test_branch_result(
            BranchId::new(),
            vec![FileChange::Added(PathBuf::from("file2.txt"))],
        );

        let conflicts = detect_conflicts(Path::new("/base"), &[branch1, branch2]).unwrap();

        assert!(conflicts.is_empty());
    }

    #[test]
    fn test_detect_conflicts_with_conflict() {
        let branch1 = create_test_branch_result(
            BranchId::new(),
            vec![FileChange::Modified(PathBuf::from("file.txt"))],
        );
        let branch2 = create_test_branch_result(
            BranchId::new(),
            vec![FileChange::Modified(PathBuf::from("file.txt"))],
        );

        let conflicts = detect_conflicts(Path::new("/base"), &[branch1, branch2]).unwrap();

        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].branch_ids().len(), 2);
    }
}
