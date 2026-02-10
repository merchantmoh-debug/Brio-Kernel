//! Diff algorithm traits and types.
//!
//! This module provides the core types and traits for diff algorithms,
//! enabling pluggable implementations with a consistent interface.

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_diff_op_is_change() {
        assert!(!DiffOp::Equal {
            old_start: 0,
            old_end: 1,
            new_start: 0,
            new_end: 1,
        }
        .is_change());

        assert!(DiffOp::Insert {
            new_start: 0,
            new_end: 1,
        }
        .is_change());

        assert!(DiffOp::Delete {
            old_start: 0,
            old_end: 1,
        }
        .is_change());

        assert!(DiffOp::Replace {
            old_start: 0,
            old_end: 1,
            new_start: 0,
            new_end: 1,
        }
        .is_change());
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

    #[test]
    fn test_diff_op_len() {
        let equal = DiffOp::Equal {
            old_start: 0,
            old_end: 5,
            new_start: 0,
            new_end: 5,
        };
        assert_eq!(equal.old_len(), 5);
        assert_eq!(equal.new_len(), 5);

        let delete = DiffOp::Delete {
            old_start: 0,
            old_end: 3,
        };
        assert_eq!(delete.old_len(), 3);
        assert_eq!(delete.new_len(), 0);

        let insert = DiffOp::Insert {
            new_start: 0,
            new_end: 4,
        };
        assert_eq!(insert.old_len(), 0);
        assert_eq!(insert.new_len(), 4);

        let replace = DiffOp::Replace {
            old_start: 0,
            old_end: 2,
            new_start: 0,
            new_end: 3,
        };
        assert_eq!(replace.old_len(), 2);
        assert_eq!(replace.new_len(), 3);
    }
}
