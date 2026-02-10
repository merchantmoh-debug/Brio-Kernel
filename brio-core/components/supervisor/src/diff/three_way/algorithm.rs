//! Three-way merge algorithm.
use crate::diff::three_way::conflict::{ChangeKind, ChangeRange};
use crate::diff::three_way::outcome::{LineConflict, MergeOutcome, ThreeWayMergeError};
use crate::diff::{DiffAlgorithm, DiffOp};
/// Performs a three-way merge.
pub fn three_way_merge<A: DiffAlgorithm + ?Sized>(
    base: &str,
    branch_a: &str,
    branch_b: &str,
    diff_algo: &A,
) -> Result<MergeOutcome, ThreeWayMergeError> {
    let (base_l, a_l, b_l) = (
        base.lines().collect::<Vec<_>>(),
        branch_a.lines().collect::<Vec<_>>(),
        branch_b.lines().collect::<Vec<_>>(),
    );
    let (diff_a, diff_b) = (diff_algo.diff(&base_l, &a_l), diff_algo.diff(&base_l, &b_l));
    let (ch_a, ch_b) = (extract_changes(&diff_a), extract_changes(&diff_b));
    Ok(perform_merge(&base_l, &a_l, &b_l, &ch_a, &ch_b))
}
/// Three-way merge with configuration.
pub fn three_way_merge_with_config<A: DiffAlgorithm>(
    base: &str,
    branch_a: &str,
    branch_b: &str,
    diff_algo: &A,
    _cfg: &crate::diff::three_way::outcome::ThreeWayConfig,
) -> Result<MergeOutcome, ThreeWayMergeError> {
    three_way_merge(base, branch_a, branch_b, diff_algo)
}
pub(crate) fn extract_changes(diff_ops: &[DiffOp]) -> Vec<ChangeRange> {
    let mut changes = Vec::new();
    for op in diff_ops {
        match op {
            DiffOp::Equal { .. } => {}
            DiffOp::Insert { new_start, new_end } => changes.push(ChangeRange {
                base_range: None,
                target_range: Some((*new_start, *new_end)),
                kind: ChangeKind::Insert,
            }),
            DiffOp::Delete { old_start, old_end } => changes.push(ChangeRange {
                base_range: Some((*old_start, *old_end)),
                target_range: None,
                kind: ChangeKind::Delete,
            }),
            DiffOp::Replace {
                old_start,
                old_end,
                new_start,
                new_end,
            } => changes.push(ChangeRange {
                base_range: Some((*old_start, *old_end)),
                target_range: Some((*new_start, *new_end)),
                kind: ChangeKind::Replace,
            }),
        }
    }
    changes
}
pub(crate) fn changes_overlap(a: &ChangeRange, b: &ChangeRange) -> bool {
    let (ar, br) = (
        a.base_range.unwrap_or(a.target_range.unwrap_or((0, 0))),
        b.base_range.unwrap_or(b.target_range.unwrap_or((0, 0))),
    );
    ar.0 < br.1 && br.0 < ar.1
}
pub(crate) fn perform_merge(
    base: &[&str],
    branch_a: &[&str],
    branch_b: &[&str],
    changes_a: &[ChangeRange],
    changes_b: &[ChangeRange],
) -> MergeOutcome {
    let (mut merged, mut conflicts, mut base_idx) = (Vec::new(), Vec::new(), 0);
    let mut all_c: Vec<(usize, &ChangeRange, char)> = changes_a
        .iter()
        .map(|c| (c.base_range.map_or(0, |r| r.0), c, 'a'))
        .chain(
            changes_b
                .iter()
                .map(|c| (c.base_range.map_or(0, |r| r.0), c, 'b')),
        )
        .collect();
    all_c.sort_by_key(|(p, _, _)| *p);
    let mut i = 0;
    while i < all_c.len() {
        let (pos, change, branch) = all_c[i];
        while base_idx < pos {
            merged.push(base[base_idx].to_string());
            base_idx += 1;
        }
        let (mut overlap, mut branches) = (vec![change], vec![branch]);
        let mut j = i + 1;
        while j < all_c.len() {
            let (_, oc, ob) = all_c[j];
            if overlap.iter().any(|c| changes_overlap(c, oc)) {
                if !branches.contains(&ob) {
                    overlap.push(oc);
                    branches.push(ob);
                }
            } else {
                break;
            }
            j += 1;
        }
        if branches.len() > 1 {
            let (bs, be) = (
                base_idx,
                overlap
                    .iter()
                    .filter_map(|c| c.base_range)
                    .map(|(_, e)| e)
                    .max()
                    .unwrap_or(base_idx),
            );
            let cb: Vec<String> = if bs < be {
                base[bs..be]
                    .iter()
                    .map(std::string::ToString::to_string)
                    .collect()
            } else {
                Vec::new()
            };
            let ac: Vec<String> = overlap
                .iter()
                .zip(branches.iter())
                .find(|(_, b)| **b == 'a')
                .and_then(|(c, _)| c.target_range)
                .map_or_else(
                    || cb.clone(),
                    |(s, e)| {
                        branch_a[s..e]
                            .iter()
                            .map(std::string::ToString::to_string)
                            .collect()
                    },
                );
            let bc: Vec<String> = overlap
                .iter()
                .zip(branches.iter())
                .find(|(_, b)| **b == 'b')
                .and_then(|(c, _)| c.target_range)
                .map_or_else(
                    || cb.clone(),
                    |(s, e)| {
                        branch_b[s..e]
                            .iter()
                            .map(std::string::ToString::to_string)
                            .collect()
                    },
                );
            conflicts.push(LineConflict::new(
                merged.len() + 1,
                merged.len() + 1,
                cb,
                ac,
                bc,
            ));
            base_idx = be;
            i = j;
        } else {
            match overlap[0].kind {
                ChangeKind::Insert => {
                    if let Some((s, e)) = overlap[0].target_range {
                        for k in s..e {
                            merged.push(branch_a[k].to_string());
                        }
                    }
                }
                ChangeKind::Delete => {
                    if let Some((_, e)) = overlap[0].base_range {
                        base_idx = e;
                    }
                }
                ChangeKind::Replace => {
                    if let Some((_, e)) = overlap[0].base_range {
                        base_idx = e;
                    }
                    if let Some((s, e)) = overlap[0].target_range {
                        let src = if branches[0] == 'a' {
                            branch_a
                        } else {
                            branch_b
                        };
                        for k in s..e {
                            merged.push(src[k].to_string());
                        }
                    }
                }
            }
            i += 1;
        }
    }
    while base_idx < base.len() {
        merged.push(base[base_idx].to_string());
        base_idx += 1;
    }
    if conflicts.is_empty() {
        MergeOutcome::Merged(merged.join("\n"))
    } else {
        for c in &mut conflicts {
            c.line_end = merged.len() + 1;
        }
        MergeOutcome::Conflicts(conflicts)
    }
}
