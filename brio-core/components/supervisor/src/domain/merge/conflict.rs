//! Conflict types for merge operations.
//!
//! This module defines types for representing merge conflicts.

use crate::domain::ids::BranchId;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Type of merge conflict.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConflictType {
    /// Content conflict - both branches modified the same file.
    Content,
    /// Delete-modify conflict - one branch deleted, other modified.
    DeleteModify,
    /// Add-add conflict - both branches added the same file.
    AddAdd,
    /// Rename conflict - file renamed differently in branches.
    Rename,
}

/// A merge conflict.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Conflict {
    file_path: PathBuf,
    kind: ConflictType,
    base_content: Option<String>,
    branch_contents: HashMap<BranchId, String>,
}

impl Conflict {
    /// Creates a new merge conflict.
    #[must_use]
    pub fn new(
        file_path: PathBuf,
        kind: ConflictType,
        base_content: Option<String>,
        branch_contents: HashMap<BranchId, String>,
    ) -> Self {
        Self {
            file_path,
            kind,
            base_content,
            branch_contents,
        }
    }

    /// Returns the path of the conflicting file.
    #[must_use]
    pub fn file_path(&self) -> &PathBuf {
        &self.file_path
    }

    /// Returns the type of conflict.
    #[must_use]
    pub const fn kind(&self) -> ConflictType {
        self.kind
    }

    /// Returns the base content (common ancestor).
    #[must_use]
    pub fn base_content(&self) -> Option<&str> {
        self.base_content.as_deref()
    }

    /// Returns the content from each conflicting branch.
    #[must_use]
    pub fn branch_contents(&self) -> &HashMap<BranchId, String> {
        &self.branch_contents
    }

    /// Checks if this is a content conflict.
    #[must_use]
    pub fn is_content_conflict(&self) -> bool {
        matches!(self.kind, ConflictType::Content)
    }

    /// Checks if this is a delete-modify conflict.
    #[must_use]
    pub fn is_delete_modify_conflict(&self) -> bool {
        matches!(self.kind, ConflictType::DeleteModify)
    }

    /// Checks if this is an add-add conflict.
    #[must_use]
    pub fn is_add_add_conflict(&self) -> bool {
        matches!(self.kind, ConflictType::AddAdd)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_conflict_type_variants() {
        assert!(matches!(ConflictType::Content, ConflictType::Content));
        assert!(matches!(
            ConflictType::DeleteModify,
            ConflictType::DeleteModify
        ));
        assert!(matches!(ConflictType::AddAdd, ConflictType::AddAdd));
        assert!(matches!(ConflictType::Rename, ConflictType::Rename));
    }

    #[test]
    fn test_conflict_creation() {
        let file_path = PathBuf::from("src/main.rs");
        let mut branch_contents = HashMap::new();
        let branch_id = BranchId::new();
        branch_contents.insert(branch_id, "content".to_string());

        let conflict = Conflict::new(
            file_path.clone(),
            ConflictType::Content,
            Some("base".to_string()),
            branch_contents.clone(),
        );

        assert_eq!(conflict.file_path(), &file_path);
        assert!(matches!(conflict.kind(), ConflictType::Content));
        assert_eq!(conflict.base_content(), Some("base"));
        assert_eq!(conflict.branch_contents(), &branch_contents);
    }

    #[test]
    fn test_conflict_type_checks() {
        let file_path = PathBuf::from("test.rs");
        let branch_contents = HashMap::new();

        let content_conflict = Conflict::new(
            file_path.clone(),
            ConflictType::Content,
            None,
            branch_contents.clone(),
        );
        assert!(content_conflict.is_content_conflict());
        assert!(!content_conflict.is_delete_modify_conflict());
        assert!(!content_conflict.is_add_add_conflict());

        let delete_modify_conflict = Conflict::new(
            file_path.clone(),
            ConflictType::DeleteModify,
            None,
            branch_contents.clone(),
        );
        assert!(!delete_modify_conflict.is_content_conflict());
        assert!(delete_modify_conflict.is_delete_modify_conflict());
        assert!(!delete_modify_conflict.is_add_add_conflict());

        let add_add_conflict =
            Conflict::new(file_path, ConflictType::AddAdd, None, branch_contents);
        assert!(!add_add_conflict.is_content_conflict());
        assert!(!add_add_conflict.is_delete_modify_conflict());
        assert!(add_add_conflict.is_add_add_conflict());
    }

    #[test]
    fn test_conflict_without_base() {
        let file_path = PathBuf::from("new_file.rs");
        let branch_contents = HashMap::new();

        let conflict = Conflict::new(file_path, ConflictType::AddAdd, None, branch_contents);
        assert_eq!(conflict.base_content(), None);
    }
}
