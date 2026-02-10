//! Conflict detection and representation for three-way merge.

/// Represents a change range in a diff.
#[derive(Debug, Clone)]
pub(crate) struct ChangeRange {
    /// Line range in base (inclusive start, exclusive end).
    pub base_range: Option<(usize, usize)>,
    /// Line range in target (inclusive start, exclusive end).
    pub target_range: Option<(usize, usize)>,
    /// Type of change.
    pub kind: ChangeKind,
}

/// Type of change in a diff operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ChangeKind {
    Insert,
    Delete,
    Replace,
}
