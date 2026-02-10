//! Myers diff algorithm optimizations.
//!
//! This module provides optimization functions for post-processing diff results.

use crate::diff::DiffOp;

/// Coalesces consecutive operations of the same type.
///
/// This function optimizes the diff output by combining adjacent operations
/// that can be merged, such as consecutive Delete followed by Insert
/// at the same position, which can be converted to a Replace operation.
pub(crate) fn coalesce_operations(ops: &mut Vec<DiffOp>) {
    if ops.len() < 2 {
        return;
    }

    let mut i = 0;
    while i < ops.len().saturating_sub(1) {
        // Check for Delete followed by Insert at the same position -> Replace
        if let (DiffOp::Delete { old_start, old_end }, DiffOp::Insert { new_start, new_end }) =
            (&ops[i], &ops[i + 1])
        {
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
    fn test_coalesce_delete_insert_to_replace() {
        let mut ops = vec![
            DiffOp::Delete {
                old_start: 0,
                old_end: 1,
            },
            DiffOp::Insert {
                new_start: 0,
                new_end: 1,
            },
        ];

        coalesce_operations(&mut ops);

        assert_eq!(ops.len(), 1);
        assert!(matches!(
            ops[0],
            DiffOp::Replace {
                old_start: 0,
                old_end: 1,
                new_start: 0,
                new_end: 1
            }
        ));
    }

    #[test]
    fn test_no_coalesce_when_not_adjacent() {
        let mut ops = vec![
            DiffOp::Insert {
                new_start: 0,
                new_end: 1,
            },
            DiffOp::Delete {
                old_start: 5,
                old_end: 6,
            },
        ];

        coalesce_operations(&mut ops);

        assert_eq!(ops.len(), 2);
    }

    #[test]
    fn test_empty_ops() {
        let mut ops: Vec<DiffOp> = vec![];
        coalesce_operations(&mut ops);
        assert!(ops.is_empty());
    }

    #[test]
    fn test_single_op() {
        let mut ops = vec![DiffOp::Delete {
            old_start: 0,
            old_end: 1,
        }];
        coalesce_operations(&mut ops);
        assert_eq!(ops.len(), 1);
    }
}
