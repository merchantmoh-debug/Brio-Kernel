//! Myers diff algorithm implementation.
//!
//! Myers' algorithm is a classic diff algorithm with O(ND) time complexity,
//! where N is the sum of the lengths of the two sequences and D is the number
//! of differences. It's particularly efficient when the two texts are similar.
//!
//! The algorithm uses a greedy approach to find the shortest edit script (SES)
//! that transforms one sequence into another.

use super::{DiffAlgorithm, DiffOp};

/// Myers diff algorithm implementation.
///
/// This implementation uses the classic O(ND) algorithm described by Eugene Myers
/// in "An O(ND) Difference Algorithm and Its Variations" (1986).
#[derive(Debug, Clone, Copy, Default)]
pub struct MyersDiff;

impl MyersDiff {
    /// Creates a new Myers diff algorithm instance.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl DiffAlgorithm for MyersDiff {
    fn diff(&self, base: &[&str], target: &[&str]) -> Vec<DiffOp> {
        if base.is_empty() && target.is_empty() {
            return Vec::new();
        }

        if base.is_empty() {
            return vec![DiffOp::Insert {
                new_start: 0,
                new_end: target.len(),
            }];
        }

        if target.is_empty() {
            return vec![DiffOp::Delete {
                old_start: 0,
                old_end: base.len(),
            }];
        }

        // Convert to a format suitable for the algorithm
        let base_len = base.len();
        let target_len = target.len();

        // Use the classic Myers algorithm
        let ses = compute_ses(base, target);

        // Convert SES to DiffOps
        convert_ses_to_diff_ops(&ses, base_len, target_len)
    }
}

/// A single edit operation in the shortest edit script.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EditOp {
    /// Insert from target (diagonal move down-right)
    Insert,
    /// Delete from base (horizontal move right)
    Delete,
    /// Keep unchanged (diagonal move down-right)
    Keep,
}

/// Computes the shortest edit script (SES) between two sequences.
///
/// This is the core Myers algorithm. It uses dynamic programming to find
/// the minimum number of edits needed to transform `base` into `target`.
fn compute_ses(base: &[&str], target: &[&str]) -> Vec<EditOp> {
    let n = base.len();
    let m = target.len();

    // Maximum possible edit distance
    let max_d = n + m;

    // If one sequence is empty, return all insertions or deletions
    if n == 0 {
        return vec![EditOp::Insert; m];
    }
    if m == 0 {
        return vec![EditOp::Delete; n];
    }

    // The frontier of the edit graph
    // v[k] stores the x value of the furthest reaching path on diagonal k
    let mut v: Vec<isize> = vec![0; 2 * max_d + 1];
    let mut trace: Vec<Vec<isize>> = Vec::new();

    'outer: for d in 0..=max_d {
        trace.push(v.clone());

        for k in -(d as isize)..=(d as isize) {
            // Only process valid diagonals (same parity as d)
            if k.abs() % 2 != d as isize % 2 {
                continue;
            }

            let k_idx = (k + max_d as isize) as usize;

            // Decide whether to go down or right
            let x: isize = if k == -(d as isize) || (k != d as isize && v[k_idx - 1] < v[k_idx + 1])
            {
                // Go down: take value from diagonal k+1
                v[k_idx + 1]
            } else {
                // Go right: take value from diagonal k-1 and add 1
                v[k_idx - 1] + 1
            };

            let y = x - k;

            // Extend the snake (diagonal moves for matching elements)
            let mut x = x;
            let mut y = y;
            while x < n as isize && y < m as isize && base[x as usize] == target[y as usize] {
                x += 1;
                y += 1;
            }

            v[k_idx] = x;

            // Check if we've reached the end
            if x >= n as isize && y >= m as isize {
                break 'outer;
            }
        }
    }

    // Backtrack to find the actual edit script
    backtrack(base, target, &trace, max_d)
}

/// Backtracks through the trace to find the edit script.
fn backtrack(base: &[&str], target: &[&str], trace: &[Vec<isize>], max_d: usize) -> Vec<EditOp> {
    let n = base.len();
    let m = target.len();

    let mut edits = Vec::new();
    let mut x = n;
    let mut y = m;

    // Work backwards through the trace
    for (d, v) in trace.iter().enumerate().rev().skip(1) {
        let d = d as isize;
        let k = x as isize - y as isize;
        let k_idx = (k + max_d as isize) as usize;

        // Determine the previous diagonal
        let prev_k = if k == -d || (k != d && v[k_idx - 1] < v[k_idx + 1]) {
            k + 1
        } else {
            k - 1
        };
        let prev_k_idx = (prev_k + max_d as isize) as usize;

        let prev_x = v[prev_k_idx] as usize;
        let prev_y = (prev_x as isize - prev_k) as usize;

        // Add edits for the horizontal/vertical move
        while x > prev_x && y > prev_y {
            // Diagonal move (keep)
            edits.push(EditOp::Keep);
            x -= 1;
            y -= 1;
        }

        if x > prev_x {
            // Horizontal move (delete)
            edits.push(EditOp::Delete);
            x -= 1;
        } else if y > prev_y {
            // Vertical move (insert)
            edits.push(EditOp::Insert);
            y -= 1;
        }
    }

    // Add remaining keeps at the beginning
    while x > 0 && y > 0 {
        edits.push(EditOp::Keep);
        x -= 1;
        y -= 1;
    }

    // Handle remaining insertions or deletions
    while y > 0 {
        edits.push(EditOp::Insert);
        y -= 1;
    }
    while x > 0 {
        edits.push(EditOp::Delete);
        x -= 1;
    }

    edits.reverse();
    edits
}

/// Converts a shortest edit script to DiffOps.
fn convert_ses_to_diff_ops(ses: &[EditOp], _base_len: usize, _target_len: usize) -> Vec<DiffOp> {
    if ses.is_empty() {
        return Vec::new();
    }

    let mut ops = Vec::new();
    let mut base_idx = 0;
    let mut target_idx = 0;
    let mut current_op: Option<DiffOp> = None;

    for edit in ses {
        match edit {
            EditOp::Keep => {
                // Finish any pending change
                if let Some(op) = current_op.take() {
                    ops.push(op);
                }

                // Start or extend an equal operation
                match ops.last_mut() {
                    Some(DiffOp::Equal {
                        old_end, new_end, ..
                    }) => {
                        *old_end = base_idx + 1;
                        *new_end = target_idx + 1;
                    }
                    _ => {
                        ops.push(DiffOp::Equal {
                            old_start: base_idx,
                            old_end: base_idx + 1,
                            new_start: target_idx,
                            new_end: target_idx + 1,
                        });
                    }
                }
                base_idx += 1;
                target_idx += 1;
            }
            EditOp::Delete => {
                // Check if we're in the middle of building a replace
                if let Some(DiffOp::Replace { old_end, .. }) = &mut current_op {
                    *old_end = base_idx + 1;
                } else if let Some(DiffOp::Delete { old_end, .. }) = &mut current_op {
                    *old_end = base_idx + 1;
                } else {
                    // Finish any pending equal operation
                    if let Some(op) = current_op.take() {
                        ops.push(op);
                    }
                    current_op = Some(DiffOp::Delete {
                        old_start: base_idx,
                        old_end: base_idx + 1,
                    });
                }
                base_idx += 1;
            }
            EditOp::Insert => {
                // Check if we're building a replace
                match &mut current_op {
                    Some(DiffOp::Delete { old_start, old_end }) => {
                        // Convert delete to replace
                        let old_start_val = *old_start;
                        let old_end_val = *old_end;
                        current_op = Some(DiffOp::Replace {
                            old_start: old_start_val,
                            old_end: old_end_val,
                            new_start: target_idx,
                            new_end: target_idx + 1,
                        });
                    }
                    Some(DiffOp::Replace { new_end, .. }) => {
                        *new_end = target_idx + 1;
                    }
                    Some(DiffOp::Insert { new_end, .. }) => {
                        *new_end = target_idx + 1;
                    }
                    _ => {
                        // Finish any pending operation
                        if let Some(op) = current_op.take() {
                            ops.push(op);
                        }
                        current_op = Some(DiffOp::Insert {
                            new_start: target_idx,
                            new_end: target_idx + 1,
                        });
                    }
                }
                target_idx += 1;
            }
        }
    }

    // Don't forget the last operation
    if let Some(op) = current_op {
        ops.push(op);
    }

    // Post-process to ensure proper Replace operations
    coalesce_operations(&mut ops);

    ops
}

/// Coalesces consecutive operations of the same type.
fn coalesce_operations(ops: &mut Vec<DiffOp>) {
    if ops.len() < 2 {
        return;
    }

    let mut i = 0;
    while i < ops.len().saturating_sub(1) {
        // Check for Delete followed by Insert at the same position -> Replace
        if let (DiffOp::Delete { old_start, old_end }, DiffOp::Insert { new_start, new_end }) =
            (&ops[i], &ops[i + 1])
        {
            // Convert to Replace
            ops[i] = DiffOp::Replace {
                old_start: *old_start,
                old_end: *old_end,
                new_start: *new_start,
                new_end: *new_end,
            };
            ops.remove(i + 1);
            continue;
        }

        // Check for Insert followed by Delete at the same position -> Replace
        if let (DiffOp::Insert { new_start, new_end }, DiffOp::Delete { old_start, old_end }) =
            (&ops[i], &ops[i + 1])
        {
            // Only if they're adjacent
            if new_end == new_start && old_start == old_end {
                ops[i] = DiffOp::Replace {
                    old_start: *old_start,
                    old_end: *old_end,
                    new_start: *new_start,
                    new_end: *new_end,
                };
                ops.remove(i + 1);
                continue;
            }
        }

        i += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_sequences() {
        let diff = MyersDiff::new();
        let result = diff.diff(&[], &[]);
        assert!(result.is_empty());
    }

    #[test]
    fn test_identical_sequences() {
        let diff = MyersDiff::new();
        let base = vec!["line1", "line2", "line3"];
        let target = vec!["line1", "line2", "line3"];
        let result = diff.diff(&base, &target);

        assert_eq!(result.len(), 1);
        assert!(matches!(
            result[0],
            DiffOp::Equal {
                old_start: 0,
                old_end: 3,
                new_start: 0,
                new_end: 3
            }
        ));
    }

    #[test]
    fn test_insertion() {
        let diff = MyersDiff::new();
        let base = vec!["line1", "line3"];
        let target = vec!["line1", "line2", "line3"];
        let result = diff.diff(&base, &target);

        // Should have: Equal(0,1,0,1), Insert(1,2), Equal(1,2,2,3)
        // Or potentially coalesced differently
        assert!(!result.is_empty());

        // Check that we have an insert operation
        let has_insert = result.iter().any(|op| matches!(op, DiffOp::Insert { .. }));
        assert!(has_insert, "Expected an Insert operation");
    }

    #[test]
    fn test_deletion() {
        let diff = MyersDiff::new();
        let base = vec!["line1", "line2", "line3"];
        let target = vec!["line1", "line3"];
        let result = diff.diff(&base, &target);

        // Should have a delete operation
        let has_delete = result.iter().any(|op| matches!(op, DiffOp::Delete { .. }));
        assert!(has_delete, "Expected a Delete operation");
    }

    #[test]
    fn test_replacement() {
        let diff = MyersDiff::new();
        let base = vec!["line1", "old", "line3"];
        let target = vec!["line1", "new", "line3"];
        let result = diff.diff(&base, &target);

        // Check that we detect the change
        let has_change = result.iter().any(|op| op.is_change());
        assert!(has_change, "Expected a change operation");
    }

    #[test]
    fn test_all_insertions() {
        let diff = MyersDiff::new();
        let base: Vec<&str> = vec![];
        let target = vec!["a", "b", "c"];
        let result = diff.diff(&base, &target);

        assert_eq!(result.len(), 1);
        assert!(matches!(
            result[0],
            DiffOp::Insert {
                new_start: 0,
                new_end: 3
            }
        ));
    }

    #[test]
    fn test_all_deletions() {
        let diff = MyersDiff::new();
        let base = vec!["a", "b", "c"];
        let target: Vec<&str> = vec![];
        let result = diff.diff(&base, &target);

        assert_eq!(result.len(), 1);
        assert!(matches!(
            result[0],
            DiffOp::Delete {
                old_start: 0,
                old_end: 3
            }
        ));
    }

    #[test]
    fn test_complex_diff() {
        let diff = MyersDiff::new();
        let base = vec!["a", "b", "c", "d", "e"];
        let target = vec!["a", "x", "c", "y", "e"];
        let result = diff.diff(&base, &target);

        // Should have operations for:
        // - Keep "a"
        // - Replace "b" with "x"
        // - Keep "c"
        // - Replace "d" with "y"
        // - Keep "e"

        assert!(!result.is_empty());

        // Verify that changes are detected
        let change_count = result.iter().filter(|op| op.is_change()).count();
        assert!(change_count >= 1, "Expected at least one change");
    }

    #[test]
    fn test_backtrack_simple() {
        let base = vec!["a", "b"];
        let target = vec!["a", "c"];
        let ses = compute_ses(&base, &target);

        // Should have: Keep, Delete, Insert (or similar)
        assert!(!ses.is_empty());
        assert!(ses.contains(&EditOp::Keep));
    }
}
