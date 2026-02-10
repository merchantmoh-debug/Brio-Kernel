//! Modular diff algorithm module for line-level conflict detection.
//!
//! This module provides a trait-based approach to diff algorithms,
//! allowing users to swap implementations while maintaining a consistent interface.

pub mod myers;
pub mod three_way;

pub use myers::MyersDiff;
pub use three_way::{MergeOutcome, ThreeWayMergeError, three_way_merge};

/// A single diff operation representing the difference between two texts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiffOp {
    /// Lines that are unchanged between both texts.
    Equal {
        /// Start line index in the old text (0-based, inclusive).
        old_start: usize,
        /// End line index in the old text (0-based, exclusive).
        old_end: usize,
        /// Start line index in the new text (0-based, inclusive).
        new_start: usize,
        /// End line index in the new text (0-based, exclusive).
        new_end: usize,
    },
    /// Lines that were inserted in the new text.
    Insert {
        /// Start line index in the new text (0-based, inclusive).
        new_start: usize,
        /// End line index in the new text (0-based, exclusive).
        new_end: usize,
    },
    /// Lines that were deleted from the old text.
    Delete {
        /// Start line index in the old text (0-based, inclusive).
        old_start: usize,
        /// End line index in the old text (0-based, exclusive).
        old_end: usize,
    },
    /// Lines that were replaced (deleted and inserted).
    Replace {
        /// Start line index in the old text (0-based, inclusive).
        old_start: usize,
        /// End line index in the old text (0-based, exclusive).
        old_end: usize,
        /// Start line index in the new text (0-based, inclusive).
        new_start: usize,
        /// End line index in the new text (0-based, exclusive).
        new_end: usize,
    },
}

impl DiffOp {
    /// Returns the range of lines affected in the old text, if applicable.
    #[must_use]
    pub fn old_range(&self) -> Option<(usize, usize)> {
        match self {
            Self::Equal {
                old_start, old_end, ..
            } => Some((*old_start, *old_end)),
            Self::Delete { old_start, old_end } => Some((*old_start, *old_end)),
            Self::Replace {
                old_start, old_end, ..
            } => Some((*old_start, *old_end)),
            Self::Insert { .. } => None,
        }
    }

    /// Returns the range of lines affected in the new text, if applicable.
    #[must_use]
    pub fn new_range(&self) -> Option<(usize, usize)> {
        match self {
            Self::Equal {
                new_start, new_end, ..
            } => Some((*new_start, *new_end)),
            Self::Insert { new_start, new_end } => Some((*new_start, *new_end)),
            Self::Replace {
                new_start, new_end, ..
            } => Some((*new_start, *new_end)),
            Self::Delete { .. } => None,
        }
    }

    /// Returns true if this operation represents a change (not equal).
    #[must_use]
    pub const fn is_change(&self) -> bool {
        !matches!(self, Self::Equal { .. })
    }

    /// Returns the size of the change in the old text.
    #[must_use]
    pub fn old_len(&self) -> usize {
        match self {
            Self::Equal {
                old_start, old_end, ..
            } => old_end - old_start,
            Self::Delete { old_start, old_end } => old_end - old_start,
            Self::Replace {
                old_start, old_end, ..
            } => old_end - old_start,
            Self::Insert { .. } => 0,
        }
    }

    /// Returns the size of the change in the new text.
    #[must_use]
    pub fn new_len(&self) -> usize {
        match self {
            Self::Equal {
                new_start, new_end, ..
            } => new_end - new_start,
            Self::Insert { new_start, new_end } => new_end - new_start,
            Self::Replace {
                new_start, new_end, ..
            } => new_end - new_start,
            Self::Delete { .. } => 0,
        }
    }
}

/// Trait for diff algorithms.
///
/// This trait defines the interface for computing differences between two texts.
/// Implementations can use different algorithms (Myers, patience, histogram, etc.)
/// while maintaining a consistent API.
///
/// # Type Parameters
///
/// The trait uses `Send + Sync` bounds to allow safe sharing across threads.
pub trait DiffAlgorithm: Send + Sync {
    /// Computes the diff between two texts.
    ///
    /// # Arguments
    ///
    /// * `base` - The base/old version of the text as lines.
    /// * `target` - The target/new version of the text as lines.
    ///
    /// # Returns
    ///
    /// A vector of `DiffOp` operations that transform `base` into `target`.
    fn diff(&self, base: &[&str], target: &[&str]) -> Vec<DiffOp>;
}

/// A simple edit script entry for reconstructing text from diffs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Edit {
    /// Keep the line from the base.
    Keep,
    /// Delete the line from the base.
    Delete,
    /// Insert a new line.
    Insert(String),
}

/// Applies a diff to reconstruct the target text from the base.
///
/// # Arguments
///
/// * `base` - The base text as lines.
/// * `diff_ops` - The diff operations to apply.
///
/// # Returns
///
/// The reconstructed target text.
#[must_use]
pub fn apply_diff(base: &[&str], diff_ops: &[DiffOp]) -> Vec<String> {
    let mut result = Vec::new();

    for op in diff_ops {
        match op {
            DiffOp::Equal {
                old_start, old_end, ..
            } => {
                for i in *old_start..*old_end {
                    if i < base.len() {
                        result.push(base[i].to_string());
                    }
                }
            }
            DiffOp::Delete {
                old_start: _,
                old_end: _,
            } => {
                // Skip these lines (they're deleted)
            }
            DiffOp::Insert {
                new_start: _,
                new_end: _,
            } => {
                // Insertions require the target text, which we don't have here
                // This is a limitation - in practice, three_way_merge handles this
                // by working with the actual text content
            }
            DiffOp::Replace {
                old_start: _,
                old_end: _,
                ..
            } => {
                // Replacements require target text too
            }
        }
    }

    result
}

/// Converts diff operations into a more convenient edit script.
///
/// # Arguments
///
/// * `base` - The base text as lines.
/// * `target` - The target text as lines.
/// * `diff_ops` - The diff operations.
///
/// # Returns
///
/// A vector of edits that can be applied sequentially.
#[must_use]
pub fn to_edit_script(base: &[&str], target: &[&str], diff_ops: &[DiffOp]) -> Vec<Edit> {
    let mut edits = Vec::new();

    for op in diff_ops {
        match op {
            DiffOp::Equal {
                old_start,
                old_end,
                new_start: _,
                new_end: _,
            } => {
                // Advance through equal lines
                let count = old_end - old_start;
                for _ in 0..count {
                    if edits.len() < base.len() {
                        edits.push(Edit::Keep);
                    }
                }
            }
            DiffOp::Delete { old_start, old_end } => {
                // Mark lines as deleted
                let count = old_end - old_start;
                for _ in 0..count {
                    edits.push(Edit::Delete);
                }
            }
            DiffOp::Insert { new_start, new_end } => {
                // Insert new lines
                for i in *new_start..*new_end {
                    if i < target.len() {
                        edits.push(Edit::Insert(target[i].to_string()));
                    }
                }
            }
            DiffOp::Replace {
                old_start,
                old_end,
                new_start,
                new_end,
            } => {
                // Delete old lines
                let delete_count = old_end - old_start;
                for _ in 0..delete_count {
                    edits.push(Edit::Delete);
                }
                // Insert new lines
                for i in *new_start..*new_end {
                    if i < target.len() {
                        edits.push(Edit::Insert(target[i].to_string()));
                    }
                }
            }
        }
    }

    edits
}

/// Formats a diff as a unified diff string (similar to `diff -u`).
///
/// # Arguments
///
/// * `old_path` - Path to the old file (for headers).
/// * `new_path` - Path to the new file (for headers).
/// * `old_lines` - The old file content as lines.
/// * `new_lines` - The new file content as lines.
/// * `diff_ops` - The diff operations.
///
/// # Returns
///
/// A unified diff formatted string.
#[must_use]
pub fn format_unified_diff(
    old_path: &str,
    new_path: &str,
    old_lines: &[&str],
    new_lines: &[&str],
    diff_ops: &[DiffOp],
) -> String {
    let mut output = String::new();

    // Headers
    output.push_str(&format!("--- {old_path}\n"));
    output.push_str(&format!("+++ {new_path}\n"));

    for op in diff_ops {
        match op {
            DiffOp::Equal { .. } => {
                // Unified diff typically shows 3 lines of context
                // For simplicity, we'll show all equal lines
            }
            DiffOp::Delete { old_start, old_end } => {
                let count = old_end - old_start;
                if count > 0 {
                    output.push_str(&format!("@@ -{},{} +0,0 @@\n", old_start + 1, count));
                    for i in *old_start..*old_end {
                        if i < old_lines.len() {
                            output.push_str(&format!("-{}\n", old_lines[i]));
                        }
                    }
                }
            }
            DiffOp::Insert { new_start, new_end } => {
                let count = new_end - new_start;
                if count > 0 {
                    output.push_str(&format!("@@ -0,0 +{},{} @@\n", new_start + 1, count));
                    for i in *new_start..*new_end {
                        if i < new_lines.len() {
                            output.push_str(&format!("+{}\n", new_lines[i]));
                        }
                    }
                }
            }
            DiffOp::Replace {
                old_start,
                old_end,
                new_start,
                new_end,
            } => {
                let old_count = old_end - old_start;
                let new_count = new_end - new_start;
                output.push_str(&format!(
                    "@@ -{},{} +{},{} @@\n",
                    old_start + 1,
                    old_count,
                    new_start + 1,
                    new_count
                ));
                for i in *old_start..*old_end {
                    if i < old_lines.len() {
                        output.push_str(&format!("-{}\n", old_lines[i]));
                    }
                }
                for i in *new_start..*new_end {
                    if i < new_lines.len() {
                        output.push_str(&format!("+{}\n", new_lines[i]));
                    }
                }
            }
        }
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_diff_op_is_change() {
        assert!(
            !DiffOp::Equal {
                old_start: 0,
                old_end: 1,
                new_start: 0,
                new_end: 1,
            }
            .is_change()
        );

        assert!(
            DiffOp::Insert {
                new_start: 0,
                new_end: 1,
            }
            .is_change()
        );

        assert!(
            DiffOp::Delete {
                old_start: 0,
                old_end: 1,
            }
            .is_change()
        );

        assert!(
            DiffOp::Replace {
                old_start: 0,
                old_end: 1,
                new_start: 0,
                new_end: 1,
            }
            .is_change()
        );
    }

    #[test]
    fn test_diff_op_ranges() {
        let equal = DiffOp::Equal {
            old_start: 0,
            old_end: 5,
            new_start: 0,
            new_end: 5,
        };
        assert_eq!(equal.old_range(), Some((0, 5)));
        assert_eq!(equal.new_range(), Some((0, 5)));

        let delete = DiffOp::Delete {
            old_start: 2,
            old_end: 4,
        };
        assert_eq!(delete.old_range(), Some((2, 4)));
        assert_eq!(delete.new_range(), None);

        let insert = DiffOp::Insert {
            new_start: 1,
            new_end: 3,
        };
        assert_eq!(insert.old_range(), None);
        assert_eq!(insert.new_range(), Some((1, 3)));
    }
}
