//! Myers diff algorithm.
use crate::diff::{DiffAlgorithm, DiffOp};
/// Myers diff algorithm.
#[derive(Debug, Clone, Copy, Default)]
pub struct MyersDiff;
impl MyersDiff {
    /// Creates new instance.
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
        convert_ses_to_diff_ops(&compute_ses(base, target), base.len(), target.len())
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EditOp {
    Insert,
    Delete,
    Keep,
}
pub(crate) fn compute_ses(base: &[&str], target: &[&str]) -> Vec<EditOp> {
    let (n, m, max_d) = (base.len(), target.len(), base.len() + target.len());
    if n == 0 {
        return vec![EditOp::Insert; m];
    }
    if m == 0 {
        return vec![EditOp::Delete; n];
    }
    let mut v: Vec<isize> = vec![0; 2 * max_d + 1];
    let mut trace: Vec<Vec<isize>> = Vec::new();
    'outer: for d in 0..=max_d {
        trace.push(v.clone());
        for k in -(d as isize)..=(d as isize) {
            if k.abs() % 2 != d as isize % 2 {
                continue;
            }
            let k_idx = (k + max_d as isize) as usize;
            let x: isize = if k == -(d as isize) || (k != d as isize && v[k_idx - 1] < v[k_idx + 1])
            {
                v[k_idx + 1]
            } else {
                v[k_idx - 1] + 1
            };
            let (mut x, mut y) = (x, x - k);
            while x < n as isize && y < m as isize && base[x as usize] == target[y as usize] {
                x += 1;
                y += 1;
            }
            v[k_idx] = x;
            if x >= n as isize && y >= m as isize {
                break 'outer;
            }
        }
    }
    backtrack(base, target, &trace, max_d)
}
pub(crate) fn backtrack(
    base: &[&str],
    target: &[&str],
    trace: &[Vec<isize>],
    max_d: usize,
) -> Vec<EditOp> {
    let (mut edits, mut x, mut y) = (Vec::new(), base.len(), target.len());
    for (d, v) in trace.iter().enumerate().rev().skip(1) {
        let (d, k, k_idx) = (
            d as isize,
            x as isize - y as isize,
            ((x as isize - y as isize) + max_d as isize) as usize,
        );
        let prev_k = if k == -d || (k != d && v[k_idx - 1] < v[k_idx + 1]) {
            k + 1
        } else {
            k - 1
        };
        let (prev_x, prev_y) = (
            v[(prev_k + max_d as isize) as usize] as usize,
            (v[(prev_k + max_d as isize) as usize] - prev_k) as usize,
        );
        while x > prev_x && y > prev_y {
            edits.push(EditOp::Keep);
            x -= 1;
            y -= 1;
        }
        if x > prev_x {
            edits.push(EditOp::Delete);
            x -= 1;
        } else if y > prev_y {
            edits.push(EditOp::Insert);
            y -= 1;
        }
    }
    while x > 0 && y > 0 {
        edits.push(EditOp::Keep);
        x -= 1;
        y -= 1;
    }
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
pub(crate) fn convert_ses_to_diff_ops(ses: &[EditOp], _bl: usize, _tl: usize) -> Vec<DiffOp> {
    if ses.is_empty() {
        return Vec::new();
    }
    let (mut ops, mut bi, mut ti, mut cur) = (Vec::new(), 0, 0, None::<DiffOp>);
    for edit in ses {
        match edit {
            EditOp::Keep => {
                if let Some(op) = cur.take() {
                    ops.push(op);
                }
                match ops.last_mut() {
                    Some(DiffOp::Equal {
                        old_end, new_end, ..
                    }) => {
                        *old_end = bi + 1;
                        *new_end = ti + 1;
                    }
                    _ => ops.push(DiffOp::Equal {
                        old_start: bi,
                        old_end: bi + 1,
                        new_start: ti,
                        new_end: ti + 1,
                    }),
                }
                bi += 1;
                ti += 1;
            }
            EditOp::Delete => {
                if let Some(DiffOp::Replace { old_end, .. } | DiffOp::Delete { old_end, .. }) =
                    &mut cur
                {
                    *old_end = bi + 1;
                } else {
                    if let Some(op) = cur.take() {
                        ops.push(op);
                    }
                    cur = Some(DiffOp::Delete {
                        old_start: bi,
                        old_end: bi + 1,
                    });
                }
                bi += 1;
            }
            EditOp::Insert => {
                match &mut cur {
                    Some(DiffOp::Delete { old_start, old_end }) => {
                        let (s, e) = (*old_start, *old_end);
                        cur = Some(DiffOp::Replace {
                            old_start: s,
                            old_end: e,
                            new_start: ti,
                            new_end: ti + 1,
                        });
                    }
                    Some(DiffOp::Replace { new_end, .. } | DiffOp::Insert { new_end, .. }) => {
                        *new_end = ti + 1;
                    }
                    _ => {
                        if let Some(op) = cur.take() {
                            ops.push(op);
                        }
                        cur = Some(DiffOp::Insert {
                            new_start: ti,
                            new_end: ti + 1,
                        });
                    }
                }
                ti += 1;
            }
        }
    }
    if let Some(op) = cur {
        ops.push(op);
    }
    super::optimization::coalesce_operations(&mut ops);
    ops
}
