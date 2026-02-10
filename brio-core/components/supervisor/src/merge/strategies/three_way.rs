//! Three-Way Merge Strategy - Line-level conflict detection.
//!
//! This module provides the `ThreeWayStrategy` with line-level conflict detection,
//! as well as the `OursStrategy` and `TheirsStrategy` for simple preference-based merging.

use async_trait::async_trait;
use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use tracing::{debug, info, warn};

use crate::diff::{DiffAlgorithm, MergeOutcome, MyersDiff, three_way_merge};
use crate::domain::BranchId;
use crate::merge::conflict::{BranchResult, Conflict, FileChange, MergeError, MergeResult};
use crate::merge::strategies::{MergeStrategy, validate_branch_count};

/// Abstraction for filesystem operations to support WASM compatibility.
///
/// This trait abstracts file reading operations to enable:
/// - WASM targets: Uses WIT filesystem bindings (session-fs-ops)
/// - Native targets: Uses standard filesystem operations
///
/// Following SOLID principles (Dependency Inversion) and allowing testability.
pub trait FileSystem: Send + Sync {
    /// Reads the entire contents of a file into a string.
    ///
    /// # Arguments
    /// * `path` - The path to the file to read
    ///
    /// # Returns
    /// * `Ok(Some(String))` - File content if file exists and is readable
    /// * `Ok(None)` - File does not exist
    /// * `Err(String)` - Error reading the file
    fn read_file(&self, path: &Path) -> Result<Option<String>, String>;

    /// Checks if a file exists at the given path.
    ///
    /// # Arguments
    /// * `path` - The path to check
    ///
    /// # Returns
    /// * `Ok(bool)` - true if file exists, false otherwise
    /// * `Err(String)` - Error checking existence
    fn file_exists(&self, path: &Path) -> Result<bool, String>;
}

/// Native filesystem implementation using standard library.
///
/// Used for native builds and testing.
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeFileSystem;

impl NativeFileSystem {
    /// Creates a new native filesystem instance.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl FileSystem for NativeFileSystem {
    fn read_file(&self, path: &Path) -> Result<Option<String>, String> {
        if !path.exists() {
            return Ok(None);
        }

        std::fs::read_to_string(path)
            .map(Some)
            .map_err(|e| format!("Failed to read file {}: {}", path.display(), e))
    }

    fn file_exists(&self, path: &Path) -> Result<bool, String> {
        Ok(path.exists())
    }
}

/// WASM-compatible filesystem implementation using WIT bindings.
///
/// This implementation uses the session-fs-ops WIT interface for filesystem
/// operations within sandboxed sessions. Used in WASM builds.
///
/// NOTE: This is currently cfg-gated as the WIT interface (session_fs_ops)
/// is not yet defined. Enable the "session-fs-ops" feature when available.
#[cfg(all(target_arch = "wasm32", feature = "session-fs-ops"))]
#[derive(Debug, Clone, Default)]
pub struct WasmFileSystem {
    /// The session ID for filesystem operations.
    /// All file operations are scoped to this session.
    session_id: String,
}

#[cfg(all(target_arch = "wasm32", feature = "session-fs-ops"))]
impl WasmFileSystem {
    /// Creates a new WASM filesystem instance with the given session ID.
    ///
    /// # Arguments
    /// * `session_id` - The session ID for sandboxed filesystem operations
    #[must_use]
    pub fn new(session_id: impl Into<String>) -> Self {
        Self {
            session_id: session_id.into(),
        }
    }

    /// Returns the session ID for this filesystem instance.
    #[must_use]
    pub fn session_id(&self) -> &str {
        &self.session_id
    }
}

#[cfg(all(target_arch = "wasm32", feature = "session-fs-ops"))]
impl FileSystem for WasmFileSystem {
    fn read_file(&self, path: &Path) -> Result<Option<String>, String> {
        use crate::brio_host::brio::core::session_fs_ops as wit;

        let path_str = path
            .to_str()
            .ok_or_else(|| format!("Invalid UTF-8 in path: {}", path.display()))?;

        match wit::read_file(&self.session_id, path_str) {
            Ok(content) => Ok(Some(content)),
            Err(e) => {
                // Check if the error indicates file not found
                if e.contains("not found") || e.contains("No such file") {
                    Ok(None)
                } else {
                    Err(format!("Failed to read file {}: {}", path.display(), e))
                }
            }
        }
    }

    fn file_exists(&self, path: &Path) -> Result<bool, String> {
        use crate::brio_host::brio::core::session_fs_ops as wit;

        let path_str = path
            .to_str()
            .ok_or_else(|| format!("Invalid UTF-8 in path: {}", path.display()))?;

        // Try to read the file - if it succeeds, the file exists
        match wit::read_file(&self.session_id, path_str) {
            Ok(_) => Ok(true),
            Err(e) => {
                if e.contains("not found") || e.contains("No such file") {
                    Ok(false)
                } else {
                    Err(format!("Failed to check file {}: {}", path.display(), e))
                }
            }
        }
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
    /// Filesystem abstraction for reading files.
    /// Uses Arc for thread safety and to allow different implementations.
    pub filesystem: Arc<dyn FileSystem>,
}

impl std::fmt::Debug for ThreeWayMergeConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ThreeWayMergeConfig")
            .field("diff_algorithm", &"<dyn DiffAlgorithm>")
            .field("branch_a_name", &self.branch_a_name)
            .field("branch_b_name", &self.branch_b_name)
            .field("include_base", &self.include_base)
            .field("filesystem", &"<dyn FileSystem>")
            .finish()
    }
}

impl Default for ThreeWayMergeConfig {
    fn default() -> Self {
        Self {
            diff_algorithm: Arc::new(MyersDiff),
            branch_a_name: "branch-a".to_string(),
            branch_b_name: "branch-b".to_string(),
            include_base: true,
            filesystem: Arc::new(NativeFileSystem::new()),
        }
    }
}

impl ThreeWayMergeConfig {
    /// Creates a new configuration with the Myers diff algorithm.
    #[must_use]
    pub fn with_myers() -> Self {
        Self {
            diff_algorithm: Arc::new(MyersDiff),
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

    /// Sets a custom filesystem implementation.
    #[must_use]
    pub fn with_filesystem<F: FileSystem + 'static>(mut self, fs: F) -> Self {
        self.filesystem = Arc::new(fs);
        self
    }
}

/// Three-way merge strategy with line-level conflict detection.
///
/// This strategy compares the actual content of files to detect line-level conflicts,
/// rather than just checking file-level change types. It uses a configurable diff
/// algorithm (default: Myers) to compare files and can merge non-overlapping changes
/// automatically while marking overlapping changes as conflicts.
pub struct ThreeWayStrategy {
    config: ThreeWayMergeConfig,
}

impl ThreeWayStrategy {
    /// Creates a new three-way merge strategy with the given configuration.
    #[must_use]
    pub fn new(config: ThreeWayMergeConfig) -> Self {
        Self { config }
    }

    /// Detects line-level conflicts for a single file.
    ///
    /// This method reads the file content from the base and branch directories
    /// and performs a three-way merge to detect line-level conflicts.
    ///
    /// Uses the filesystem abstraction to support WASM environments where
    /// standard filesystem operations are not available.
    fn detect_line_level_conflict(
        &self,
        base_path: &Path,
        branch_a: &BranchResult,
        branch_b: &BranchResult,
        file_path: &Path,
    ) -> Option<Conflict> {
        let filesystem = &self.config.filesystem;

        // Read base file content using filesystem abstraction
        let base_file = base_path.join(file_path);
        let base_content = match filesystem.read_file(&base_file) {
            Ok(Some(content)) => content,
            Ok(None) => String::new(), // File doesn't exist
            Err(e) => {
                warn!("Failed to read base file {}: {}", base_file.display(), e);
                return None; // Fall back to file-level conflict detection
            }
        };

        // Read branch A file content using filesystem abstraction
        let file_a = branch_a.path.join(file_path);
        let content_a = match filesystem.read_file(&file_a) {
            Ok(Some(content)) => content,
            Ok(None) => String::new(), // File doesn't exist
            Err(e) => {
                warn!("Failed to read branch A file {}: {}", file_a.display(), e);
                return None; // Fall back to file-level conflict detection
            }
        };

        // Read branch B file content using filesystem abstraction
        let file_b = branch_b.path.join(file_path);
        let content_b = match filesystem.read_file(&file_b) {
            Ok(Some(content)) => content,
            Ok(None) => String::new(), // File doesn't exist
            Err(e) => {
                warn!("Failed to read branch B file {}: {}", file_b.display(), e);
                return None; // Fall back to file-level conflict detection
            }
        };

        // Perform three-way merge
        match three_way_merge(
            &base_content,
            &content_a,
            &content_b,
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
                        "Line-level conflict in {}: {} conflict(s) detected",
                        file_path.display(),
                        line_conflicts.len()
                    ),
                    first_conflict.line_start(),
                    first_conflict.line_end(),
                    base_content,
                    content_a,
                    content_b,
                ))
            }
            Err(_) => {
                // If three-way merge fails, fall back to file-level conflict
                Some(Conflict::new(
                    file_path.to_path_buf(),
                    vec![branch_a.branch_id, branch_b.branch_id],
                    format!(
                        "Failed to perform line-level merge for {}",
                        file_path.display()
                    ),
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
    fn name(&self) -> &'static str {
        "three-way"
    }

    fn description(&self) -> &'static str {
        "Three-way merge with line-level conflict detection using configurable diff algorithm"
    }

    async fn merge(
        &self,
        base_path: &Path,
        branches: &[BranchResult],
    ) -> Result<MergeResult, MergeError> {
        validate_branch_count(branches)?;

        if branches.len() < 2 {
            // Not enough branches for a three-way merge
            return Ok(MergeResult::success(
                branches.iter().flat_map(|b| b.changes.clone()).collect(),
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

                    let (id_a, change_a) = &changes[0];
                    let (id_b, change_b) = &changes[1];

                    // Find the branch results
                    let branch_a = branches
                        .iter()
                        .find(|b| b.branch_id == *id_a)
                        .ok_or(MergeError::BranchNotFound(*id_a))?;
                    let branch_b = branches
                        .iter()
                        .find(|b| b.branch_id == *id_b)
                        .ok_or(MergeError::BranchNotFound(*id_b))?;

                    // Check if it's a modification conflict (need line-level detection)
                    if matches!(
                        (change_a, change_b),
                        (FileChange::Modified(_), FileChange::Modified(_))
                    ) {
                        // Perform line-level conflict detection
                        if let Some(conflict) =
                            self.detect_line_level_conflict(base_path, branch_a, branch_b, &path)
                        {
                            warn!("Line-level conflict detected at {:?}", path);
                            conflicts.push(conflict);
                        } else {
                            // Changes don't overlap - can be auto-merged
                            debug!("Changes at {:?} are non-overlapping - auto-merged", path);
                            merged_changes.push(change_a.clone());
                        }
                    } else {
                        // Other types of conflicts (addition/deletion) - mark as file-level conflict
                        warn!(
                            "File-level conflict at {:?} - incompatible change types",
                            path
                        );
                        let branch_ids: Vec<BranchId> = changes.iter().map(|(id, _)| *id).collect();
                        conflicts.push(Conflict::new(
                            path.clone(),
                            branch_ids,
                            format!("Incompatible change types for {}", path.display()),
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

/// Always prefer base version on conflict.
pub struct OursStrategy;

#[async_trait]
impl MergeStrategy for OursStrategy {
    fn name(&self) -> &'static str {
        "ours"
    }

    fn description(&self) -> &'static str {
        "Always prefer the base version when conflicts occur"
    }

    async fn merge(
        &self,
        _base_path: &Path,
        branches: &[BranchResult],
    ) -> Result<MergeResult, MergeError> {
        validate_branch_count(branches)?;

        info!(
            "Applying 'ours' merge strategy to {} branches",
            branches.len()
        );

        // Collect all unique changes from branches
        let mut all_changes: HashMap<PathBuf, FileChange> = HashMap::new();
        let mut conflicts = Vec::new();

        for branch in branches {
            for change in &branch.changes {
                let path = change.path().to_path_buf();

                match all_changes.get(&path) {
                    Some(existing) => {
                        // Conflict detected - prefer ours (base), so we skip the branch change
                        if crate::merge::conflict::changes_conflict(existing, change) {
                            warn!(
                                "Conflict detected at {:?} between branches, preferring base (ours)",
                                path
                            );
                            conflicts.push(Conflict::new(
                                path.clone(),
                                vec![branch.branch_id],
                                format!("Conflict at {} - using base version", path.display()),
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
    fn name(&self) -> &'static str {
        "theirs"
    }

    fn description(&self) -> &'static str {
        "Always prefer the branch version when conflicts occur"
    }

    async fn merge(
        &self,
        _base_path: &Path,
        branches: &[BranchResult],
    ) -> Result<MergeResult, MergeError> {
        validate_branch_count(branches)?;

        info!(
            "Applying 'theirs' merge strategy to {} branches",
            branches.len()
        );

        // Collect all changes, preferring later branches
        let mut all_changes: HashMap<PathBuf, FileChange> = HashMap::new();
        let mut conflicts = Vec::new();

        for branch in branches {
            for change in &branch.changes {
                let path = change.path().to_path_buf();

                if all_changes
                    .get(&path)
                    .filter(|e| crate::merge::conflict::changes_conflict(e, change))
                    .is_some()
                {
                    warn!(
                        "Conflict detected at {:?} - preferring branch {} (theirs)",
                        path, branch.branch_id
                    );
                    conflicts.push(Conflict::new(
                        path.clone(),
                        vec![branch.branch_id],
                        format!("Conflict at {} - using branch version", path.display()),
                    ));
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
