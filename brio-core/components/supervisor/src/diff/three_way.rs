//! Three-way merge algorithm implementation.
//!
//! This module provides three-way merge functionality that can detect
//! line-level conflicts by comparing a base version with two branch versions.

use std::path::PathBuf;
use thiserror::Error;

use super::{DiffAlgorithm, DiffOp};

/// Errors that can occur during three-way merge operations.
#[derive(Debug, Error, Clone)]
pub enum ThreeWayMergeError {
    /// Invalid input (e.g., non-text files).
    #[error("Invalid input for three-way merge: {0}")]
    InvalidInput(String),

    /// Binary files cannot be merged using text-based algorithms.
    #[error("Cannot merge binary files: {0}")]
    BinaryFile(PathBuf),

    /// Maximum file size exceeded.
    #[error("File too large: {path} (max {max_size} bytes, got {actual_size})")]
    FileTooLarge {
        /// Path to the file.
        path: PathBuf,
        /// Maximum allowed size in bytes.
        max_size: usize,
        /// Actual file size in bytes.
        actual_size: usize,
    },
}

/// The result of a three-way merge operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MergeOutcome {
    /// The merge was successful with no conflicts.
    Merged(String),
    /// The merge has conflicts that need manual resolution.
    Conflicts(Vec<LineConflict>),
}

/// Represents a conflict at the line level.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineConflict {
    line_start: usize,
    line_end: usize,
    base_lines: Vec<String>,
    branch_a_lines: Vec<String>,
    branch_b_lines: Vec<String>,
}

impl LineConflict {
    /// Creates a new line conflict.
    #[must_use]
    pub fn new(
        line_start: usize,
        line_end: usize,
        base_lines: Vec<String>,
        branch_a_lines: Vec<String>,
        branch_b_lines: Vec<String>,
    ) -> Self {
        Self {
            line_start,
            line_end,
            base_lines,
            branch_a_lines,
            branch_b_lines,
        }
    }

    /// Returns the start line number in the merged file (1-based).
    #[must_use]
    pub const fn line_start(&self) -> usize {
        self.line_start
    }

    /// Returns the end line number in the merged file (1-based, exclusive).
    #[must_use]
    pub const fn line_end(&self) -> usize {
        self.line_end
    }

    /// Returns lines from the base version, if applicable.
    #[must_use]
    pub fn base_lines(&self) -> &[String] {
        &self.base_lines
    }

    /// Returns lines from branch A.
    #[must_use]
    pub fn branch_a_lines(&self) -> &[String] {
        &self.branch_a_lines
    }

    /// Returns lines from branch B.
    #[must_use]
    pub fn branch_b_lines(&self) -> &[String] {
        &self.branch_b_lines
    }

    /// Formats the conflict using Git-style conflict markers.
    #[must_use]
    pub fn format_with_markers(&self, branch_a_name: &str, branch_b_name: &str) -> String {
        let mut output = String::new();

        // Opening marker with branch A identifier
        output.push_str(&format!("<<<<<<< {}", branch_a_name));
        if !self.branch_a_lines.is_empty() {
            output.push('\n');
            for line in &self.branch_a_lines {
                output.push_str(line);
                output.push('\n');
            }
        } else {
            output.push('\n');
        }

        // Separator with base content (if available)
        if !self.base_lines.is_empty() {
            output.push_str("||||||| base\n");
            for line in &self.base_lines {
                output.push_str(line);
                output.push('\n');
            }
        }

        // Separator
        output.push_str("=======\n");

        // Branch B content
        if !self.branch_b_lines.is_empty() {
            for line in &self.branch_b_lines {
                output.push_str(line);
                output.push('\n');
            }
        }

        // Closing marker
        output.push_str(&format!(">>>>>>> {}\n", branch_b_name));

        output
    }
}

/// Performs a three-way merge using the provided diff algorithm.
///
/// This function compares a base version with two branch versions to determine
/// what changes each branch made, then attempts to merge them automatically.
/// Changes that don't overlap are merged; overlapping changes are marked as conflicts.
///
/// # Arguments
///
/// * `base` - The base/common ancestor content.
/// * `branch_a` - The content from branch A.
/// * `branch_b` - The content from branch B.
/// * `diff_algo` - The diff algorithm to use for computing differences.
///
/// # Returns
///
/// A `MergeOutcome` containing either the merged content or a list of conflicts.
///
/// # Errors
///
/// Returns an error if the input is invalid (e.g., binary files).
pub fn three_way_merge<A: DiffAlgorithm + ?Sized>(
    base: &str,
    branch_a: &str,
    branch_b: &str,
    diff_algo: &A,
) -> Result<MergeOutcome, ThreeWayMergeError> {
    // Split into lines (preserving line endings for accurate reconstruction)
    let base_lines: Vec<&str> = base.lines().collect();
    let branch_a_lines: Vec<&str> = branch_a.lines().collect();
    let branch_b_lines: Vec<&str> = branch_b.lines().collect();

    // Compute diffs from base to each branch
    let diff_a = diff_algo.diff(&base_lines, &branch_a_lines);
    let diff_b = diff_algo.diff(&base_lines, &branch_b_lines);

    // Convert diffs to change ranges
    let changes_a = extract_changes(&diff_a);
    let changes_b = extract_changes(&diff_b);

    // Perform the merge
    let result = perform_merge(
        &base_lines,
        &branch_a_lines,
        &branch_b_lines,
        &changes_a,
        &changes_b,
    );

    Ok(result)
}

/// Represents a change range in a diff.
#[derive(Debug, Clone)]
struct ChangeRange {
    /// Line range in base (inclusive start, exclusive end).
    base_range: Option<(usize, usize)>,
    /// Line range in target (inclusive start, exclusive end).
    target_range: Option<(usize, usize)>,
    /// Type of change.
    kind: ChangeKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ChangeKind {
    Insert,
    Delete,
    Replace,
}

/// Extracts non-equal (change) ranges from a diff.
fn extract_changes(diff_ops: &[DiffOp]) -> Vec<ChangeRange> {
    let mut changes = Vec::new();

    for op in diff_ops {
        match op {
            DiffOp::Equal { .. } => {
                // Skip equal sections
            }
            DiffOp::Insert { new_start, new_end } => {
                changes.push(ChangeRange {
                    base_range: None,
                    target_range: Some((*new_start, *new_end)),
                    kind: ChangeKind::Insert,
                });
            }
            DiffOp::Delete { old_start, old_end } => {
                changes.push(ChangeRange {
                    base_range: Some((*old_start, *old_end)),
                    target_range: None,
                    kind: ChangeKind::Delete,
                });
            }
            DiffOp::Replace {
                old_start,
                old_end,
                new_start,
                new_end,
            } => {
                changes.push(ChangeRange {
                    base_range: Some((*old_start, *old_end)),
                    target_range: Some((*new_start, *new_end)),
                    kind: ChangeKind::Replace,
                });
            }
        }
    }

    changes
}

/// Checks if two change ranges overlap in the base.
fn changes_overlap(a: &ChangeRange, b: &ChangeRange) -> bool {
    // Get base ranges
    let a_range = a.base_range.unwrap_or_else(|| {
        let target = a.target_range.unwrap_or((0, 0));
        (target.0, target.1)
    });
    let b_range = b.base_range.unwrap_or_else(|| {
        let target = b.target_range.unwrap_or((0, 0));
        (target.0, target.1)
    });

    // Two ranges overlap if they share any common lines
    // [a_start, a_end) and [b_start, b_end) overlap if:
    // a_start < b_end && b_start < a_end
    a_range.0 < b_range.1 && b_range.0 < a_range.1
}

/// Performs the actual three-way merge.
fn perform_merge(
    base: &[&str],
    branch_a: &[&str],
    branch_b: &[&str],
    changes_a: &[ChangeRange],
    changes_b: &[ChangeRange],
) -> MergeOutcome {
    let mut merged_lines: Vec<String> = Vec::new();
    let mut conflicts: Vec<LineConflict> = Vec::new();
    let mut base_idx = 0;

    // Collect all change positions and sort them
    let mut all_changes: Vec<(
        usize,
        &ChangeRange,
        char, // 'a' or 'b'
    )> = Vec::new();

    for change in changes_a {
        let pos = change.base_range.map(|r| r.0).unwrap_or(0);
        all_changes.push((pos, change, 'a'));
    }

    for change in changes_b {
        let pos = change.base_range.map(|r| r.0).unwrap_or(0);
        all_changes.push((pos, change, 'b'));
    }

    // Sort by position
    all_changes.sort_by_key(|(pos, _, _)| *pos);

    // Process changes in order
    let mut i = 0;
    while i < all_changes.len() {
        let (pos, change, branch) = all_changes[i];

        // Add unchanged lines before this change
        while base_idx < pos {
            merged_lines.push(base[base_idx].to_string());
            base_idx += 1;
        }

        // Look for overlapping changes
        let mut overlapping: Vec<&ChangeRange> = vec![change];
        let mut branches: Vec<char> = vec![branch];

        let mut j = i + 1;
        while j < all_changes.len() {
            let (_, other_change, other_branch) = all_changes[j];

            // Check if this overlaps with any of our collected changes
            let overlaps = overlapping.iter().any(|c| changes_overlap(c, other_change));

            if overlaps {
                // Avoid adding the same branch twice
                if !branches.contains(&other_branch) {
                    overlapping.push(other_change);
                    branches.push(other_branch);
                }
            } else {
                break;
            }
            j += 1;
        }

        if branches.len() > 1 {
            // Conflict: both branches changed the same region
            let base_start = base_idx;
            let base_end = overlapping
                .iter()
                .filter_map(|c| c.base_range)
                .map(|(_, end)| end)
                .max()
                .unwrap_or(base_start);

            let conflict_base: Vec<String> = if base_start < base_end {
                base[base_start..base_end]
                    .iter()
                    .map(|s| s.to_string())
                    .collect()
            } else {
                Vec::new()
            };

            // Get content from branch A
            let a_content: Vec<String> = if let Some(a_change) = overlapping
                .iter()
                .zip(branches.iter())
                .find(|(_, b)| **b == 'a')
                .map(|(c, _)| c)
            {
                if let Some((start, end)) = a_change.target_range {
                    branch_a[start..end].iter().map(|s| s.to_string()).collect()
                } else {
                    Vec::new() // Deleted
                }
            } else {
                // Branch A didn't change this region, use base
                conflict_base.clone()
            };

            // Get content from branch B
            let b_content: Vec<String> = if let Some(b_change) = overlapping
                .iter()
                .zip(branches.iter())
                .find(|(_, b)| **b == 'b')
                .map(|(c, _)| c)
            {
                if let Some((start, end)) = b_change.target_range {
                    branch_b[start..end].iter().map(|s| s.to_string()).collect()
                } else {
                    Vec::new() // Deleted
                }
            } else {
                // Branch B didn't change this region, use base
                conflict_base.clone()
            };

            conflicts.push(LineConflict::new(
                merged_lines.len() + 1,
                merged_lines.len() + 1, // Will be updated later
                conflict_base,
                a_content,
                b_content,
            ));

            // Advance indices past the conflict
            base_idx = base_end;
            i = j;
        } else {
            // No conflict, apply the change
            let change = overlapping[0];

            match change.kind {
                ChangeKind::Insert => {
                    if let Some((start, end)) = change.target_range {
                        for k in start..end {
                            merged_lines.push(branch_a[k].to_string());
                        }
                    }
                }
                ChangeKind::Delete => {
                    if let Some((_, end)) = change.base_range {
                        base_idx = end;
                    }
                }
                ChangeKind::Replace => {
                    if let Some((_, end)) = change.base_range {
                        base_idx = end;
                    }
                    if let Some((start, end)) = change.target_range {
                        let branch = branches[0];
                        let source = if branch == 'a' { branch_a } else { branch_b };
                        for k in start..end {
                            merged_lines.push(source[k].to_string());
                        }
                    }
                }
            }

            i += 1;
        }
    }

    // Add remaining unchanged lines
    while base_idx < base.len() {
        merged_lines.push(base[base_idx].to_string());
        base_idx += 1;
    }

    if conflicts.is_empty() {
        MergeOutcome::Merged(merged_lines.join("\n"))
    } else {
        // Update line_end for each conflict
        for conflict in &mut conflicts {
            conflict.line_end = merged_lines.len() + 1;
        }
        MergeOutcome::Conflicts(conflicts)
    }
}

/// Configuration for three-way merge operations.
#[derive(Debug, Clone)]
pub struct ThreeWayConfig {
    max_file_size: usize,
    allow_binary: bool,
    branch_a_name: String,
    branch_b_name: String,
}

impl ThreeWayConfig {
    /// Creates a new configuration for three-way merge operations.
    pub fn new(
        max_file_size: usize,
        allow_binary: bool,
        branch_a_name: impl Into<String>,
        branch_b_name: impl Into<String>,
    ) -> Self {
        Self {
            max_file_size,
            allow_binary,
            branch_a_name: branch_a_name.into(),
            branch_b_name: branch_b_name.into(),
        }
    }

    /// Returns the maximum file size in bytes (default: 10MB).
    #[must_use]
    pub const fn max_file_size(&self) -> usize {
        self.max_file_size
    }

    /// Returns whether to allow binary file merging.
    #[must_use]
    pub const fn allow_binary(&self) -> bool {
        self.allow_binary
    }

    /// Returns the branch A name for conflict markers.
    #[must_use]
    pub fn branch_a_name(&self) -> &str {
        &self.branch_a_name
    }

    /// Returns the branch B name for conflict markers.
    #[must_use]
    pub fn branch_b_name(&self) -> &str {
        &self.branch_b_name
    }
}

impl Default for ThreeWayConfig {
    fn default() -> Self {
        Self {
            max_file_size: 10 * 1024 * 1024, // 10MB
            allow_binary: false,
            branch_a_name: "branch-a".to_string(),
            branch_b_name: "branch-b".to_string(),
        }
    }
}

/// Three-way merge with configuration options.
pub fn three_way_merge_with_config<A: DiffAlgorithm>(
    base: &str,
    branch_a: &str,
    branch_b: &str,
    diff_algo: &A,
    _config: &ThreeWayConfig,
) -> Result<MergeOutcome, ThreeWayMergeError> {
    // For now, just delegate to the main function
    // In the future, config options can be applied here
    three_way_merge(base, branch_a, branch_b, diff_algo)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diff::MyersDiff;

    #[test]
    fn test_no_conflict_different_regions() {
        let base = "line1\nline2\nline3\nline4\nline5";
        let branch_a = "line1\nmodified2\nline3\nline4\nline5";
        let branch_b = "line1\nline2\nline3\nmodified4\nline5";

        let result = three_way_merge(base, branch_a, branch_b, &MyersDiff::new()).unwrap();

        match result {
            MergeOutcome::Merged(content) => {
                assert!(content.contains("modified2"));
                assert!(content.contains("modified4"));
            }
            MergeOutcome::Conflicts(_) => panic!("Expected no conflicts"),
        }
    }

    #[test]
    fn test_conflict_same_region() {
        let base = "line1\nline2\nline3";
        let branch_a = "line1\nmodified-a\nline3";
        let branch_b = "line1\nmodified-b\nline3";

        let result = three_way_merge(base, branch_a, branch_b, &MyersDiff::new()).unwrap();

        match result {
            MergeOutcome::Conflicts(conflicts) => {
                assert_eq!(conflicts.len(), 1);
                assert_eq!(conflicts[0].branch_a_lines, vec!["modified-a"]);
                assert_eq!(conflicts[0].branch_b_lines, vec!["modified-b"]);
            }
            MergeOutcome::Merged(_) => panic!("Expected conflicts"),
        }
    }

    #[test]
    fn test_no_conflict_same_change() {
        let base = "line1\nline2\nline3";
        let branch_a = "line1\nmodified\nline3";
        let branch_b = "line1\nmodified\nline3";

        let result = three_way_merge(base, branch_a, branch_b, &MyersDiff::new()).unwrap();

        match result {
            MergeOutcome::Merged(content) => {
                assert!(content.contains("modified"));
            }
            MergeOutcome::Conflicts(_) => panic!("Expected no conflicts for identical changes"),
        }
    }

    #[test]
    fn test_insertion_conflict() {
        let base = "line1\nline3";
        let branch_a = "line1\ninsertion-a\nline3";
        let branch_b = "line1\ninsertion-b\nline3";

        let result = three_way_merge(base, branch_a, branch_b, &MyersDiff::new()).unwrap();

        match result {
            MergeOutcome::Conflicts(conflicts) => {
                assert!(!conflicts.is_empty());
            }
            MergeOutcome::Merged(_) => panic!("Expected conflicts for different insertions"),
        }
    }

    #[test]
    fn test_deletion_conflict() {
        let base = "line1\nline2\nline3";
        let branch_a = "line1\nline3"; // deleted line2
        let branch_b = "line1\nmodified\nline3"; // modified line2

        let result = three_way_merge(base, branch_a, branch_b, &MyersDiff::new()).unwrap();

        match result {
            MergeOutcome::Conflicts(conflicts) => {
                assert!(!conflicts.is_empty());
            }
            MergeOutcome::Merged(_) => panic!("Expected conflicts for deletion vs modification"),
        }
    }

    #[test]
    fn test_line_conflict_formatting() {
        let conflict = LineConflict::new(
            1,
            10,
            vec!["base-line".to_string()],
            vec!["branch-a-line".to_string()],
            vec!["branch-b-line".to_string()],
        );

        let formatted = conflict.format_with_markers("branch-a", "branch-b");

        assert!(formatted.contains("<<<<<<< branch-a"));
        assert!(formatted.contains("branch-a-line"));
        assert!(formatted.contains("||||||| base"));
        assert!(formatted.contains("base-line"));
        assert!(formatted.contains("======="));
        assert!(formatted.contains("branch-b-line"));
        assert!(formatted.contains(">>>>>>> branch-b"));
    }

    #[test]
    fn test_empty_base() {
        let base = "";
        let branch_a = "line1\nline2";
        let branch_b = "line1\nline2";

        let result = three_way_merge(base, branch_a, branch_b, &MyersDiff::new()).unwrap();

        match result {
            MergeOutcome::Merged(content) => {
                assert!(content.contains("line1"));
                assert!(content.contains("line2"));
            }
            MergeOutcome::Conflicts(_) => panic!("Expected no conflicts for identical additions"),
        }
    }

    #[test]
    fn test_addition_in_different_locations() {
        let base = "middle";
        let branch_a = "start\nmiddle";
        let branch_b = "middle\nend";

        let result = three_way_merge(base, branch_a, branch_b, &MyersDiff::new()).unwrap();

        match result {
            MergeOutcome::Merged(content) => {
                assert!(content.contains("start"));
                assert!(content.contains("middle"));
                assert!(content.contains("end"));
            }
            MergeOutcome::Conflicts(_) => {
                panic!("Expected no conflicts for non-overlapping additions")
            }
        }
    }
}
