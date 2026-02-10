//! Branching orchestrator configuration for the Brio kernel.
//!
//! This module defines parallel execution branch settings.

use serde::Deserialize;

/// Branching settings for the orchestrator.
#[derive(Debug, Deserialize, Clone)]
pub struct BranchingSettings {
    /// Maximum number of concurrent branches (default: 8)
    #[serde(default = "default_max_branches")]
    pub max_concurrent_branches: usize,

    /// Default merge strategy (default: "union")
    #[serde(default = "default_merge_strategy")]
    pub default_merge_strategy: String,

    /// Enable nested branches (default: true)
    #[serde(default = "default_true")]
    pub allow_nested_branches: bool,

    /// Merge behavior settings
    #[serde(default)]
    pub merge_settings: MergeSettings,

    /// Timeout for branch execution in seconds (default: 300)
    #[serde(default = "default_branch_timeout_secs")]
    pub branch_timeout_secs: u64,

    /// Enable line-level diff for conflict detection (default: true)
    #[serde(default = "default_true")]
    pub line_level_diffs: bool,

    /// Maximum branch nesting depth (default: 3)
    #[serde(default = "default_max_nesting_depth")]
    pub max_nesting_depth: usize,
}

/// Settings controlling merge behavior.
#[derive(Debug, Deserialize, Clone)]
pub struct MergeSettings {
    /// Auto-merge on success (default: false - requires approval)
    #[serde(default = "default_false")]
    pub auto_merge: bool,

    /// Default approval requirement for merges (default: true)
    #[serde(default = "default_true")]
    pub require_approval: bool,
}

impl Default for MergeSettings {
    fn default() -> Self {
        Self {
            auto_merge: default_false(),
            require_approval: default_true(),
        }
    }
}

impl BranchingSettings {
    /// Returns the `auto_merge` setting.
    #[must_use]
    pub fn auto_merge(&self) -> bool {
        self.merge_settings.auto_merge
    }

    /// Returns the `require_merge_approval` setting.
    #[must_use]
    pub fn require_merge_approval(&self) -> bool {
        self.merge_settings.require_approval
    }
}

impl Default for BranchingSettings {
    fn default() -> Self {
        Self {
            max_concurrent_branches: default_max_branches(),
            default_merge_strategy: default_merge_strategy(),
            allow_nested_branches: default_true(),
            merge_settings: MergeSettings::default(),
            branch_timeout_secs: default_branch_timeout_secs(),
            line_level_diffs: default_true(),
            max_nesting_depth: default_max_nesting_depth(),
        }
    }
}

fn default_max_branches() -> usize {
    8
}

fn default_merge_strategy() -> String {
    "union".to_string()
}

fn default_true() -> bool {
    true
}

fn default_false() -> bool {
    false
}

fn default_branch_timeout_secs() -> u64 {
    300
}

fn default_max_nesting_depth() -> usize {
    3
}
