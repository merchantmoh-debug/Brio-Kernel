//! Merge strategies for combining changes from multiple branches.
//!
//! This module provides different strategies for merging file changes from multiple
//! branches, including conflict detection and resolution approaches.

use async_trait::async_trait;
use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use thiserror::Error;
use tracing::{debug, info, warn};

use crate::diff::{DiffAlgorithm, three_way_merge, MyersDiff, MergeOutcome};
use crate::domain::BranchId;

/// Unique identifier for a merge operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MergeId(pub uuid::Uuid);

impl MergeId {
    /// Creates a new unique merge ID.
    #[must_use]
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4())
    }

    /// Creates a MergeId from an existing UUID.
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
    /// Returns the path associated with this change.
    #[must_use]
    pub fn path(&self) -> &Path {
        match self {
            Self::Modified(p) | Self::Added(p) | Self::Deleted(p) => p,
        }
    }

    /// Returns true if this change is a deletion.
    #[must_use]
    pub const fn is_deletion(&self) -> bool {
        matches!(self, Self::Deleted(_))
    }

    /// Returns true if this change is an addition.
    #[must_use]
    pub const fn is_addition(&self) -> bool {
        matches!(self, Self::Added(_))
    }

    /// Returns true if this change is a modification.
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
    /// The file path where the conflict occurred.
    pub path: PathBuf,
    /// The branches involved in the conflict.
    pub branch_ids: Vec<BranchId>,
    /// Description of the conflict.
    pub description: String,
    /// Start line number where the conflict begins (1-based, 0 if not applicable).
    pub line_start: usize,
    /// End line number where the conflict ends (1-based, 0 if not applicable).
    pub line_end: usize,
    /// Content from the base version (empty if not a line-level conflict).
    pub base_content: String,
    /// Content from branch A (empty if not a line-level conflict).
    pub branch_a_content: String,
    /// Content from branch B (empty if not a line-level conflict).
    pub branch_b_content: String,
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
    pub fn with_line_info(
        path: PathBuf,
        branch_ids: Vec<BranchId>,
        description: impl Into<String>,
        line_start: usize,
        line_end: usize,
        base_content: impl Into<String>,
        branch_a_content: impl Into<String>,
        branch_b_content: impl Into<String>,
    ) -> Self {
        Self {
            path,
            branch_ids,
            description: description.into(),
            line_start,
            line_end,
            base_content: base_content.into(),
            branch_a_content: branch_a_content.into(),
            branch_b_content: branch_b_content.into(),
        }
    }

    /// Returns true if this is a line-level conflict with detailed information.
    #[must_use]
    pub const fn has_line_info(&self) -> bool {
        self.line_start > 0
    }

    /// Formats the conflict using Git-style conflict markers.
    ///
    /// This is useful for displaying conflicts to users or writing them to files.
    #[must_use]
    pub fn format_with_markers(&self, branch_a_name: &str, branch_b_name: &str) -> String {
        if !self.has_line_info() {
            return format!("File-level conflict at {:?}", self.path);
        }

        let mut output = String::new();
        
        // Opening marker
        output.push_str(&format!("<<<<<<< {}\n", branch_a_name));
        output.push_str(&self.branch_a_content);
        if !self.branch_a_content.ends_with('\n') && !self.branch_a_content.is_empty() {
            output.push('\n');
        }
        
        // Base content marker (if available)
        if !self.base_content.is_empty() {
            output.push_str("||||||| base\n");
            output.push_str(&self.base_content);
            if !self.base_content.ends_with('\n') {
                output.push('\n');
            }
        }
        
        // Separator
        output.push_str("=======\n");
        
        // Branch B content
        output.push_str(&self.branch_b_content);
        if !self.branch_b_content.ends_with('\n') && !self.branch_b_content.is_empty() {
            output.push('\n');
        }
        
        // Closing marker
        output.push_str(&format!(">>>>>>> {}\n", branch_b_name));
        
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

/// Trait for merge strategies.
#[async_trait]
pub trait MergeStrategy: Send + Sync {
    /// Returns the name of the strategy.
    fn name(&self) -> &str;
    /// Returns a description of the strategy.
    fn description(&self) -> &str;

    /// Merges changes from multiple branches.
    ///
    /// # Errors
    ///
    /// Returns an error if the merge cannot be completed due to I/O errors,
    /// diff computation failures, or if the strategy cannot handle the input.
    async fn merge(
        &self,
        base_path: &Path,
        branches: &[BranchResult],
    ) -> Result<MergeResult, MergeError>;
}

/// Always prefer base version on conflict.
pub struct OursStrategy;

#[async_trait]
impl MergeStrategy for OursStrategy {
    fn name(&self) -> &str {
        "ours"
    }

    fn description(&self) -> &str {
        "Always prefer the base version when conflicts occur"
    }

    async fn merge(
        &self,
        _base_path: &Path,
        branches: &[BranchResult],
    ) -> Result<MergeResult, MergeError> {
        if branches.len() > 8 {
            return Err(MergeError::TooManyBranches(branches.len()));
        }

        info!("Applying 'ours' merge strategy to {} branches", branches.len());

        // Collect all unique changes from branches
        let mut all_changes: HashMap<PathBuf, FileChange> = HashMap::new();
        let mut conflicts = Vec::new();

        for branch in branches {
            for change in &branch.changes {
                let path = change.path().to_path_buf();
                
                match all_changes.get(&path) {
                    Some(existing) => {
                        // Conflict detected - prefer ours (base), so we skip the branch change
                        if changes_conflict(existing, change) {
                            warn!(
                                "Conflict detected at {:?} between branches, preferring base (ours)",
                                path
                            );
                            conflicts.push(Conflict::new(
                                path.clone(),
                                vec![branch.branch_id],
                                format!("Conflict at {:?} - using base version", path),
                            ));
                        }
                        // Keep the existing (first) change
                    }
                    None => {
                        all_changes.insert(path, change.clone());
                    }
                }
            }
        }

        let merged_changes: Vec<FileChange> = all_changes.into_values().collect();
        
        debug!(
            "OursStrategy: {} changes, {} conflicts",
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

/// Always prefer branch version on conflict.
pub struct TheirsStrategy;

#[async_trait]
impl MergeStrategy for TheirsStrategy {
    fn name(&self) -> &str {
        "theirs"
    }

    fn description(&self) -> &str {
        "Always prefer the branch version when conflicts occur"
    }

    async fn merge(
        &self,
        _base_path: &Path,
        branches: &[BranchResult],
    ) -> Result<MergeResult, MergeError> {
        if branches.len() > 8 {
            return Err(MergeError::TooManyBranches(branches.len()));
        }

        info!("Applying 'theirs' merge strategy to {} branches", branches.len());

        // Collect all changes, preferring later branches
        let mut all_changes: HashMap<PathBuf, FileChange> = HashMap::new();
        let mut conflicts = Vec::new();

        for branch in branches {
            for change in &branch.changes {
                let path = change.path().to_path_buf();
                
                if let Some(existing) = all_changes.get(&path) {
                    if changes_conflict(existing, change) {
                        warn!(
                            "Conflict detected at {:?} - preferring branch {} (theirs)",
                            path, branch.branch_id
                        );
                        conflicts.push(Conflict::new(
                            path.clone(),
                            vec![branch.branch_id],
                            format!("Conflict at {:?} - using branch version", path),
                        ));
                    }
                }
                // Always insert/replace with the current branch's change
                all_changes.insert(path, change.clone());
            }
        }

        let merged_changes: Vec<FileChange> = all_changes.into_values().collect();
        
        debug!(
            "TheirsStrategy: {} changes, {} conflicts",
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

/// Combines non-conflicting changes. Marks conflicts when multiple branches
/// modify the same file.
pub struct UnionStrategy;

#[async_trait]
impl MergeStrategy for UnionStrategy {
    fn name(&self) -> &str {
        "union"
    }

    fn description(&self) -> &str {
        "Combine non-conflicting changes, mark conflicts when multiple branches modify the same file"
    }

    async fn merge(
        &self,
        _base_path: &Path,
        branches: &[BranchResult],
    ) -> Result<MergeResult, MergeError> {
        if branches.len() > 8 {
            return Err(MergeError::TooManyBranches(branches.len()));
        }

        if branches.is_empty() {
            return Ok(MergeResult::success(Vec::new(), self.name()));
        }

        info!("Applying 'union' merge strategy to {} branches", branches.len());

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
                        if changes_conflict(first_change, change) {
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
                                "Multiple branches ({}) modified {:?}",
                                changes.len(),
                                path
                            ),
                        ));
                    } else {
                        // Changes don't conflict (e.g., same modification) - use first
                        debug!("File {:?} modified by multiple branches but no conflict", path);
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

/// Configuration for three-way merge operations.
#[derive(Clone)]
pub struct ThreeWayMergeConfig {
    /// The diff algorithm to use (wrapped in Arc for thread safety).
    pub diff_algorithm: Arc<dyn DiffAlgorithm>,
    /// Name for branch A in conflict markers.
    pub branch_a_name: String,
    /// Name for branch B in conflict markers.
    pub branch_b_name: String,
    /// Whether to include base content in conflict markers.
    pub include_base: bool,
}

impl std::fmt::Debug for ThreeWayMergeConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ThreeWayMergeConfig")
            .field("diff_algorithm", &"<dyn DiffAlgorithm>")
            .field("branch_a_name", &self.branch_a_name)
            .field("branch_b_name", &self.branch_b_name)
            .field("include_base", &self.include_base)
            .finish()
    }
}

impl Default for ThreeWayMergeConfig {
    fn default() -> Self {
        Self {
            diff_algorithm: Arc::new(MyersDiff::default()),
            branch_a_name: "branch-a".to_string(),
            branch_b_name: "branch-b".to_string(),
            include_base: true,
        }
    }
}

impl ThreeWayMergeConfig {
    /// Creates a new configuration with the Myers diff algorithm.
    #[must_use]
    pub fn with_myers() -> Self {
        Self {
            diff_algorithm: Arc::new(MyersDiff::default()),
            ..Default::default()
        }
    }

    /// Sets a custom diff algorithm.
    #[must_use]
    pub fn with_algorithm<A: DiffAlgorithm + 'static>(mut self, algo: A) -> Self {
        self.diff_algorithm = Arc::new(algo);
        self
    }

    /// Sets the branch names for conflict markers.
    #[must_use]
    pub fn with_branch_names(mut self, a: impl Into<String>, b: impl Into<String>) -> Self {
        self.branch_a_name = a.into();
        self.branch_b_name = b.into();
        self
    }
}

/// Three-way merge strategy with line-level conflict detection.
///
/// This strategy compares the actual content of files to detect line-level conflicts,
/// rather than just checking file-level change types. It uses a configurable diff
/// algorithm (default: Myers) to compare files and can merge non-overlapping changes
/// automatically while marking overlapping changes as conflicts.
///
/// # Example
///
/// ```rust
/// use brio_supervisor::merge::{ThreeWayStrategy, ThreeWayMergeConfig};
///
/// let config = ThreeWayMergeConfig::with_myers()
///     .with_branch_names("main", "feature");
/// let strategy = ThreeWayStrategy::new(config);
/// ```
pub struct ThreeWayStrategy {
    config: ThreeWayMergeConfig,
}

impl ThreeWayStrategy {
    /// Creates a new three-way merge strategy with the given configuration.
    #[must_use]
    pub fn new(config: ThreeWayMergeConfig) -> Self {
        Self { config }
    }

    /// Creates a new strategy with default configuration (Myers diff).
    #[must_use]
    pub fn default() -> Self {
        Self::new(ThreeWayMergeConfig::default())
    }

    /// Detects line-level conflicts for a single file.
    ///
    /// This method reads the file content from the base and branch directories
    /// and performs a three-way merge to detect line-level conflicts.
    ///
    /// # Arguments
    ///
    /// * `base_path` - Path to the base directory.
    /// * `branch_a` - First branch result containing the file.
    /// * `branch_b` - Second branch result containing the file.
    /// * `file_path` - Relative path to the file.
    ///
    /// # Returns
    ///
    /// An optional conflict if line-level conflicts are detected, or None if the
    /// file can be merged automatically.
    fn detect_line_level_conflict(
        &self,
        base_path: &Path,
        branch_a: &BranchResult,
        branch_b: &BranchResult,
        file_path: &Path,
    ) -> Option<Conflict> {
        // Read base file content
        let base_file = base_path.join(file_path);
        let base_content = if base_file.exists() {
            std::fs::read_to_string(&base_file).ok()?
        } else {
            String::new()
        };

        // Read branch A file content
        let branch_a_file = branch_a.path.join(file_path);
        let branch_a_content = if branch_a_file.exists() {
            std::fs::read_to_string(&branch_a_file).ok()?
        } else {
            String::new()
        };

        // Read branch B file content
        let branch_b_file = branch_b.path.join(file_path);
        let branch_b_content = if branch_b_file.exists() {
            std::fs::read_to_string(&branch_b_file).ok()?
        } else {
            String::new()
        };

        // Perform three-way merge
        match three_way_merge(
            &base_content,
            &branch_a_content,
            &branch_b_content,
            self.config.diff_algorithm.as_ref(),
        ) {
            Ok(MergeOutcome::Merged(_)) => None, // No conflict
            Ok(MergeOutcome::Conflicts(line_conflicts)) => {
                if line_conflicts.is_empty() {
                    return None;
                }

                // Use the first conflict's line numbers
                let first_conflict = &line_conflicts[0];
                let branch_ids = vec![branch_a.branch_id, branch_b.branch_id];

                Some(Conflict::with_line_info(
                    file_path.to_path_buf(),
                    branch_ids,
                    format!(
                        "Line-level conflict in {:?}: {} conflict(s) detected",
                        file_path,
                        line_conflicts.len()
                    ),
                    first_conflict.line_start(),
                    first_conflict.line_end(),
                    base_content,
                    branch_a_content,
                    branch_b_content,
                ))
            }
            Err(_) => {
                // If three-way merge fails, fall back to file-level conflict
                Some(Conflict::new(
                    file_path.to_path_buf(),
                    vec![branch_a.branch_id, branch_b.branch_id],
                    format!("Failed to perform line-level merge for {:?}", file_path),
                ))
            }
        }
    }
}

impl Default for ThreeWayStrategy {
    fn default() -> Self {
        Self::new(ThreeWayMergeConfig::default())
    }
}

#[async_trait]
impl MergeStrategy for ThreeWayStrategy {
    fn name(&self) -> &str {
        "three-way"
    }

    fn description(&self) -> &str {
        "Three-way merge with line-level conflict detection using configurable diff algorithm"
    }

    async fn merge(
        &self,
        base_path: &Path,
        branches: &[BranchResult],
    ) -> Result<MergeResult, MergeError> {
        if branches.len() > 8 {
            return Err(MergeError::TooManyBranches(branches.len()));
        }

        if branches.len() < 2 {
            // Not enough branches for a three-way merge
            return Ok(MergeResult::success(
                branches
                    .iter()
                    .flat_map(|b| b.changes.clone())
                    .collect(),
                self.name(),
            ));
        }

        info!(
            "Applying 'three-way' merge strategy to {} branches with line-level detection",
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
                2 => {
                    // Two branches changed this file - perform line-level merge
                    debug!(
                        "File {:?} changed by two branches - performing line-level merge",
                        path
                    );

                    let (branch_a_id, change_a) = &changes[0];
                    let (branch_b_id, change_b) = &changes[1];

                    // Find the branch results
                    let branch_a = branches
                        .iter()
                        .find(|b| b.branch_id == *branch_a_id)
                        .ok_or_else(|| MergeError::BranchNotFound(*branch_a_id))?;
                    let branch_b = branches
                        .iter()
                        .find(|b| b.branch_id == *branch_b_id)
                        .ok_or_else(|| MergeError::BranchNotFound(*branch_b_id))?;

                    // Check if it's a modification conflict (need line-level detection)
                    if matches!(
                        (change_a, change_b),
                        (FileChange::Modified(_), FileChange::Modified(_))
                    ) {
                        // Perform line-level conflict detection
                        match self.detect_line_level_conflict(base_path, branch_a, branch_b, &path) {
                            Some(conflict) => {
                                warn!(
                                    "Line-level conflict detected at {:?}",
                                    path
                                );
                                conflicts.push(conflict);
                            }
                            None => {
                                // Changes don't overlap - can be auto-merged
                                debug!("Changes at {:?} are non-overlapping - auto-merged", path);
                                merged_changes.push(change_a.clone());
                            }
                        }
                    } else {
                        // Other types of conflicts (addition/deletion) - mark as file-level conflict
                        warn!(
                            "File-level conflict at {:?} - incompatible change types",
                            path
                        );
                        let branch_ids: Vec<BranchId> =
                            changes.iter().map(|(id, _)| *id).collect();
                        conflicts.push(Conflict::new(
                            path.clone(),
                            branch_ids,
                            format!("Incompatible change types for {:?}", path),
                        ));
                    }
                }
                _ => {
                    // More than two branches - mark as conflict for simplicity
                    warn!(
                        "File {:?} changed by {} branches - marking as conflict",
                        path,
                        changes.len()
                    );
                    let branch_ids: Vec<BranchId> =
                        changes.iter().map(|(id, _)| *id).collect();
                    conflicts.push(Conflict::new(
                        path.clone(),
                        branch_ids,
                        format!(
                            "Multiple branches ({}) modified {:?}",
                            changes.len(),
                            path
                        ),
                    ));
                }
            }
        }

        info!(
            "ThreeWayStrategy: {} changes, {} conflicts",
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

/// Registry for looking up merge strategies by name.
pub struct MergeStrategyRegistry {
    strategies: HashMap<String, Box<dyn MergeStrategy>>,
}

impl std::fmt::Debug for MergeStrategyRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MergeStrategyRegistry")
            .field("strategies", &self.strategies.keys().collect::<Vec<_>>())
            .finish()
    }
}

impl Default for MergeStrategyRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl MergeStrategyRegistry {
    /// Creates a new registry with default strategies registered.
    #[must_use]
    pub fn new() -> Self {
        let mut registry = Self {
            strategies: HashMap::new(),
        };
        registry.register(Box::new(OursStrategy));
        registry.register(Box::new(TheirsStrategy));
        registry.register(Box::new(UnionStrategy));
        registry.register(Box::new(ThreeWayStrategy::default()));
        registry
    }

    /// Registers a new strategy.
    pub fn register(&mut self, strategy: Box<dyn MergeStrategy>) {
        let name = strategy.name().to_string();
        debug!("Registering merge strategy: {}", name);
        self.strategies.insert(name, strategy);
    }

    /// Gets a strategy by name.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&dyn MergeStrategy> {
        self.strategies.get(name).map(|s| s.as_ref())
    }

    /// Returns the default strategy (union).
    #[must_use]
    pub fn default_strategy(&self) -> &dyn MergeStrategy {
        self.get("union")
            .expect("Union strategy must always be registered")
    }

    /// Returns a list of all registered strategy names.
    #[must_use]
    pub fn available_strategies(&self) -> Vec<&str> {
        self.strategies.keys().map(|s| s.as_str()).collect()
    }
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
                    format!("Multiple branches have conflicting changes for {:?}", path),
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

    match (change1, change2) {
        // Two modifications to the same file conflict
        (FileChange::Modified(_), FileChange::Modified(_)) => true,
        // Deletion conflicts with any other change
        (FileChange::Deleted(_), _) | (_, FileChange::Deleted(_)) => true,
        // Two additions to the same path conflict (would overwrite)
        (FileChange::Added(_), FileChange::Added(_)) => true,
        // Other combinations are conflicts
        _ => true,
    }
}

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
        .filter(|&&b| b != b'\n' && b != b'\r' && b != b'\t' && (b < 32 || b > 126))
        .count();

    let ratio = if bytes_read > 0 {
        non_printable as f64 / bytes_read as f64
    } else {
        0.0
    };

    // If more than 30% non-printable, consider it binary
    Ok(ratio > 0.3)
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
    fn test_branch_id_creation() {
        let id1 = BranchId::new();
        let id2 = BranchId::new();
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

    #[tokio::test]
    async fn test_ours_strategy_single_branch() {
        let strategy = OursStrategy;
        let branch_id = BranchId::new();
        let changes = vec![FileChange::Added(PathBuf::from("file.txt"))];
        let branch = create_test_branch_result(branch_id, changes);
        
        let result = strategy.merge(Path::new("/base"), &[branch]).await.unwrap();
        
        assert!(!result.has_conflicts());
        assert_eq!(result.merged_changes.len(), 1);
    }

    #[tokio::test]
    async fn test_ours_strategy_prefers_base() {
        let strategy = OursStrategy;
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
        
        let result = strategy.merge(Path::new("/base"), &[branch1, branch2]).await.unwrap();
        
        // Should detect conflict but still include the change
        assert!(!result.merged_changes.is_empty());
    }

    #[tokio::test]
    async fn test_theirs_strategy_prefers_branch() {
        let strategy = TheirsStrategy;
        let branch1_id = BranchId::new();
        let branch2_id = BranchId::new();
        
        let branch1 = create_test_branch_result(
            branch1_id,
            vec![FileChange::Modified(PathBuf::from("file.txt"))],
        );
        let branch2 = create_test_branch_result(
            branch2_id,
            vec![FileChange::Added(PathBuf::from("file.txt"))],
        );
        
        let result = strategy.merge(Path::new("/base"), &[branch1, branch2]).await.unwrap();
        
        // Should have the last branch's change
        assert_eq!(result.merged_changes.len(), 1);
        assert!(result.merged_changes[0].is_addition());
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
        
        let result = strategy.merge(Path::new("/base"), &[branch1, branch2]).await.unwrap();
        
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
        
        let result = strategy.merge(Path::new("/base"), &[branch1, branch2]).await.unwrap();
        
        assert!(result.has_conflicts());
        assert_eq!(result.conflicts.len(), 1);
        assert_eq!(result.conflicts[0].branch_ids.len(), 2);
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
    async fn test_too_many_branches_error() {
        let strategy = UnionStrategy;
        let branches: Vec<BranchResult> = (0..9)
            .map(|i| {
                create_test_branch_result(
                    BranchId::new(),
                    vec![FileChange::Added(PathBuf::from(format!("file{}.txt", i)))],
                )
            })
            .collect();
        
        let result = strategy.merge(Path::new("/base"), &branches).await;
        
        assert!(matches!(result, Err(MergeError::TooManyBranches(9))));
    }

    #[test]
    fn test_registry_default_strategies() {
        let registry = MergeStrategyRegistry::new();
        
        assert!(registry.get("ours").is_some());
        assert!(registry.get("theirs").is_some());
        assert!(registry.get("union").is_some());
    }

    #[test]
    fn test_registry_default_strategy_is_union() {
        let registry = MergeStrategyRegistry::new();
        
        assert_eq!(registry.default_strategy().name(), "union");
    }

    #[test]
    fn test_registry_available_strategies() {
        let registry = MergeStrategyRegistry::new();
        let strategies = registry.available_strategies();
        
        assert_eq!(strategies.len(), 3);
        assert!(strategies.contains(&"ours"));
        assert!(strategies.contains(&"theirs"));
        assert!(strategies.contains(&"union"));
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
        assert_eq!(conflicts[0].branch_ids.len(), 2);
    }

    #[test]
    fn test_detect_conflicts_deletion_modification() {
        let branch1 = create_test_branch_result(
            BranchId::new(),
            vec![FileChange::Deleted(PathBuf::from("file.txt"))],
        );
        let branch2 = create_test_branch_result(
            BranchId::new(),
            vec![FileChange::Modified(PathBuf::from("file.txt"))],
        );
        
        let conflicts = detect_conflicts(Path::new("/base"), &[branch1, branch2]).unwrap();
        
        assert_eq!(conflicts.len(), 1);
    }

    #[test]
    fn test_conflict_creation() {
        let path = PathBuf::from("test.txt");
        let branch_ids = vec![BranchId::new(), BranchId::new()];
        let conflict = Conflict::new(path.clone(), branch_ids.clone(), "test description");
        
        assert_eq!(conflict.path, path);
        assert_eq!(conflict.branch_ids.len(), 2);
        assert_eq!(conflict.description, "test description");
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
        
        let result = strategy.merge(Path::new("/base"), &[branch1, branch2, branch3]).await.unwrap();
        
        // Should have 3 non-conflicting additions + 1 conflict
        assert_eq!(result.merged_changes.len(), 3);
        assert_eq!(result.conflicts.len(), 1);
        assert!(result.conflicts[0].path == PathBuf::from("shared.txt"));
    }

    #[tokio::test]
    async fn test_ours_strategy_with_deletions() {
        let strategy = OursStrategy;
        let branch1 = create_test_branch_result(
            BranchId::new(),
            vec![FileChange::Deleted(PathBuf::from("file.txt"))],
        );
        let branch2 = create_test_branch_result(
            BranchId::new(),
            vec![FileChange::Modified(PathBuf::from("file.txt"))],
        );
        
        let result = strategy.merge(Path::new("/base"), &[branch1, branch2]).await.unwrap();
        
        // Should report conflicts but keep first change (deletion)
        assert!(!result.conflicts.is_empty());
    }
}
