//! Union Merge Strategy - Combines non-conflicting changes.
//!
//! This module provides the `UnionStrategy` which combines non-conflicting
/// changes from multiple branches and marks conflicts when multiple branches
/// modify the same file.
use async_trait::async_trait;
use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;
use tracing::{debug, info, warn};

use crate::domain::BranchId;
use crate::merge::conflict::{BranchResult, Conflict, FileChange, MergeError, MergeResult};
use crate::merge::strategies::{MergeStrategy, validate_branch_count};

/// Combines non-conflicting changes. Marks conflicts when multiple branches
/// modify the same file.
pub struct UnionStrategy;

#[async_trait]
impl MergeStrategy for UnionStrategy {
    fn name(&self) -> &'static str {
        "union"
    }

    fn description(&self) -> &'static str {
        "Combine non-conflicting changes, mark conflicts when multiple branches modify the same file"
    }

    async fn merge(
        &self,
        _base_path: &Path,
        branches: &[BranchResult],
    ) -> Result<MergeResult, MergeError> {
        validate_branch_count(branches)?;

        if branches.is_empty() {
            return Ok(MergeResult::success(Vec::new(), self.name()));
        }

        info!(
            "Applying 'union' merge strategy to {} branches",
            branches.len()
        );

        // Track which branches changed each file
        let mut file_changes: HashMap<PathBuf, Vec<(BranchId, FileChange)>> = HashMap::new();

        for branch in branches {
            for change in &branch.changes {
                let path = change.path().to_path_buf();
                file_changes
                    .entry(path)
                    .or_default()
                    .push((branch.branch_id, change.clone()));
            }
        }

        let mut merged_changes = Vec::new();
        let mut conflicts = Vec::new();

        for (path, changes) in file_changes {
            match changes.len() {
                0 => unreachable!(),
                1 => {
                    // Only one branch changed this file - safe to include
                    debug!("File {:?} changed by single branch - including", path);
                    merged_changes.push(changes.into_iter().next().unwrap().1);
                }
                _ => {
                    // Multiple branches changed this file - check for actual conflict
                    let first_change = &changes[0].1;
                    let mut has_conflict = false;

                    for (_, change) in &changes[1..] {
                        if crate::merge::conflict::changes_conflict(first_change, change) {
                            has_conflict = true;
                            break;
                        }
                    }

                    if has_conflict {
                        warn!(
                            "Conflict detected at {:?} - {} branches modified this file",
                            path,
                            changes.len()
                        );
                        let branch_ids: Vec<BranchId> = changes.iter().map(|(id, _)| *id).collect();
                        conflicts.push(Conflict::new(
                            path.clone(),
                            branch_ids,
                            format!(
                                "Multiple branches ({}) modified {}",
                                changes.len(),
                                path.display()
                            ),
                        ));
                    } else {
                        // Changes don't conflict (e.g., same modification) - use first
                        debug!(
                            "File {:?} modified by multiple branches but no conflict",
                            path
                        );
                        merged_changes.push(changes.into_iter().next().unwrap().1);
                    }
                }
            }
        }

        info!(
            "UnionStrategy: {} changes, {} conflicts",
            merged_changes.len(),
            conflicts.len()
        );

        Ok(MergeResult::with_conflicts(
            merged_changes,
            conflicts,
            self.name(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::BranchId;
    use crate::merge::conflict::FileChange;
    use std::path::PathBuf;

    fn create_test_branch_result(id: BranchId, changes: Vec<FileChange>) -> BranchResult {
        BranchResult {
            branch_id: id,
            path: PathBuf::from("/tmp/test"),
            changes,
        }
    }

    #[tokio::test]
    async fn test_union_strategy_no_conflict_different_files() {
        let strategy = UnionStrategy;
        let branch1_id = BranchId::new();
        let branch2_id = BranchId::new();

        let branch1 = create_test_branch_result(
            branch1_id,
            vec![FileChange::Added(PathBuf::from("file1.txt"))],
        );
        let branch2 = create_test_branch_result(
            branch2_id,
            vec![FileChange::Added(PathBuf::from("file2.txt"))],
        );

        let result = strategy
            .merge(Path::new("/base"), &[branch1, branch2])
            .await
            .unwrap();

        assert!(!result.has_conflicts());
        assert_eq!(result.merged_changes.len(), 2);
    }

    #[tokio::test]
    async fn test_union_strategy_conflict_same_file() {
        let strategy = UnionStrategy;
        let branch1_id = BranchId::new();
        let branch2_id = BranchId::new();

        let branch1 = create_test_branch_result(
            branch1_id,
            vec![FileChange::Modified(PathBuf::from("file.txt"))],
        );
        let branch2 = create_test_branch_result(
            branch2_id,
            vec![FileChange::Modified(PathBuf::from("file.txt"))],
        );

        let result = strategy
            .merge(Path::new("/base"), &[branch1, branch2])
            .await
            .unwrap();

        assert!(result.has_conflicts());
        assert_eq!(result.conflicts.len(), 1);
        assert_eq!(result.conflicts[0].branch_ids().len(), 2);
    }

    #[tokio::test]
    async fn test_union_strategy_empty_branches() {
        let strategy = UnionStrategy;
        let result = strategy.merge(Path::new("/base"), &[]).await.unwrap();

        assert!(!result.has_conflicts());
        assert!(result.merged_changes.is_empty());
    }

    #[tokio::test]
    async fn test_union_strategy_single_branch() {
        let strategy = UnionStrategy;
        let branch = create_test_branch_result(
            BranchId::new(),
            vec![
                FileChange::Added(PathBuf::from("file1.txt")),
                FileChange::Modified(PathBuf::from("file2.txt")),
            ],
        );

        let result = strategy.merge(Path::new("/base"), &[branch]).await.unwrap();

        assert!(!result.has_conflicts());
        assert_eq!(result.merged_changes.len(), 2);
    }

    #[tokio::test]
    async fn test_multiple_branches_complex_merge() {
        let strategy = UnionStrategy;
        let branch1_id = BranchId::new();
        let branch2_id = BranchId::new();
        let branch3_id = BranchId::new();

        let branch1 = create_test_branch_result(
            branch1_id,
            vec![
                FileChange::Added(PathBuf::from("new1.txt")),
                FileChange::Modified(PathBuf::from("shared.txt")),
            ],
        );
        let branch2 = create_test_branch_result(
            branch2_id,
            vec![
                FileChange::Added(PathBuf::from("new2.txt")),
                FileChange::Modified(PathBuf::from("shared.txt")),
            ],
        );
        let branch3 = create_test_branch_result(
            branch3_id,
            vec![FileChange::Added(PathBuf::from("new3.txt"))],
        );

        let result = strategy
            .merge(Path::new("/base"), &[branch1, branch2, branch3])
            .await
            .unwrap();

        // Should have 3 non-conflicting additions + 1 conflict
        assert_eq!(result.merged_changes.len(), 3);
        assert_eq!(result.conflicts.len(), 1);
        assert!(*result.conflicts[0].path() == PathBuf::from("shared.txt"));
    }
}
