//! Branch Repository - CRUD Operations
//!
//! This module implements the `BranchRepository` trait for `WitBranchRepository`.

use crate::domain::{BranchId, BranchRecord, BranchStatus, MergeRequest};
use crate::merge::MergeId;
use crate::repository::branch::traits::{BranchRepository, BranchRepositoryError};
use crate::repository::branch::wit_impl::WitBranchRepository;
use crate::repository::column::BRANCH_COLUMNS;
use crate::repository::column::branch_cols;
use crate::repository::transaction::{Transaction, TransactionError, Transactional};
use crate::wit_bindings;
use chrono::Utc;

impl BranchRepository for WitBranchRepository {
    fn create_branch(&self, branch: &BranchRecord) -> Result<BranchId, BranchRepositoryError> {
        let sql = format!(
            "INSERT INTO branches ({BRANCH_COLUMNS}) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?) \
             RETURNING {}",
            branch_cols::ID
        );

        let status_json = serde_json::to_string(&branch.status())?;
        let config_json = serde_json::to_string(branch.config())?;

        let parent_id_str = branch
            .parent_id()
            .map_or_else(|| "NULL".to_string(), |id| id.inner().to_string());

        let completed_at_str = branch.completed_at().map_or_else(
            || "NULL".to_string(),
            |ts| {
                chrono::DateTime::from_timestamp(ts, 0)
                    .map_or_else(|| "NULL".to_string(), |dt| dt.to_rfc3339())
            },
        );

        let created_at_dt =
            chrono::DateTime::from_timestamp(branch.created_at(), 0).ok_or_else(|| {
                BranchRepositoryError::ParseError("Invalid created_at timestamp".to_string())
            })?;

        let params = vec![
            branch.id().inner().to_string(),
            parent_id_str,
            branch.session_id().to_string(),
            branch.name().to_string(),
            status_json,
            config_json,
            created_at_dt.to_rfc3339(),
            completed_at_str,
        ];

        let rows = wit_bindings::sql_state::query(&sql, &params)
            .map_err(BranchRepositoryError::SqlError)?;

        let row = rows.first().ok_or_else(|| {
            BranchRepositoryError::SqlError("INSERT failed to return any rows".to_string())
        })?;

        let id_val = if let Some(idx) = row.columns.iter().position(|c| c == branch_cols::ID) {
            row.values.get(idx).ok_or_else(|| {
                BranchRepositoryError::ParseError("Returned row missing id value".to_string())
            })?
        } else {
            row.values.first().ok_or_else(|| {
                BranchRepositoryError::ParseError("Returned row has no values".to_string())
            })?
        };

        let uuid = uuid::Uuid::parse_str(id_val)?;
        Ok(BranchId::from_uuid(uuid))
    }

    fn get_branch(&self, id: BranchId) -> Result<Option<BranchRecord>, BranchRepositoryError> {
        let sql = format!(
            "SELECT {BRANCH_COLUMNS} \
             FROM branches \
             WHERE {} = ?",
            branch_cols::ID
        );

        let params = vec![id.inner().to_string()];

        let rows = wit_bindings::sql_state::query(&sql, &params)
            .map_err(BranchRepositoryError::SqlError)?;

        if rows.is_empty() {
            return Ok(None);
        }

        let row = &rows[0];
        Ok(Some(Self::parse_branch_row(&row.columns, &row.values)?))
    }

    fn update_branch_status(
        &self,
        id: BranchId,
        status: BranchStatus,
    ) -> Result<(), BranchRepositoryError> {
        let sql = if status.is_terminal() {
            "UPDATE branches SET status_json = ?, completed_at = ? WHERE id = ?"
        } else {
            "UPDATE branches SET status_json = ? WHERE id = ?"
        };

        let status_json = serde_json::to_string(&status)?;

        let params = if status.is_terminal() {
            vec![status_json, Utc::now().to_rfc3339(), id.inner().to_string()]
        } else {
            vec![status_json, id.inner().to_string()]
        };

        let affected = wit_bindings::sql_state::execute(sql, &params)
            .map_err(BranchRepositoryError::SqlError)?;

        Self::expect_affected(id, affected)
    }

    fn list_active_branches(&self) -> Result<Vec<BranchRecord>, BranchRepositoryError> {
        let sql = format!(
            "SELECT {BRANCH_COLUMNS} \
             FROM branches \
             ORDER BY {} DESC",
            branch_cols::CREATED_AT
        );

        let rows =
            wit_bindings::sql_state::query(&sql, &[]).map_err(BranchRepositoryError::SqlError)?;

        rows.iter()
            .map(|row| Self::parse_branch_row(&row.columns, &row.values))
            .filter(|branch: &Result<BranchRecord, BranchRepositoryError>| {
                if let Ok(branch) = branch {
                    branch.is_active()
                } else {
                    true
                }
            })
            .collect()
    }

    fn list_branches_by_parent(
        &self,
        parent_id: BranchId,
    ) -> Result<Vec<BranchRecord>, BranchRepositoryError> {
        let sql = format!(
            "SELECT {BRANCH_COLUMNS} \
             FROM branches \
             WHERE {} = ? \
             ORDER BY {} DESC",
            branch_cols::PARENT_ID,
            branch_cols::CREATED_AT
        );

        let params = vec![parent_id.inner().to_string()];

        let rows = wit_bindings::sql_state::query(&sql, &params)
            .map_err(BranchRepositoryError::SqlError)?;

        rows.iter()
            .map(|row| Self::parse_branch_row(&row.columns, &row.values))
            .collect()
    }

    fn delete_branch(&self, id: BranchId) -> Result<(), BranchRepositoryError> {
        let sql = "DELETE FROM branches WHERE id = ?";

        let params = vec![id.inner().to_string()];

        let affected = wit_bindings::sql_state::execute(sql, &params)
            .map_err(BranchRepositoryError::SqlError)?;

        Self::expect_affected(id, affected)
    }

    fn create_merge_request(
        &self,
        branch_id: BranchId,
        parent_id: Option<BranchId>,
        strategy: &str,
    ) -> Result<MergeId, BranchRepositoryError> {
        let sql = "INSERT INTO merge_queue (id, branch_id, parent_id, strategy, status, requires_approval, approved_by, approved_at, created_at) \
                   VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?) \
                   RETURNING id";

        let merge_id = MergeId::new();
        let created_at = Utc::now();
        let parent_id_str =
            parent_id.map_or_else(|| "NULL".to_string(), |id| id.inner().to_string());

        let params = vec![
            merge_id.to_string(),
            branch_id.inner().to_string(),
            parent_id_str,
            strategy.to_string(),
            "pending".to_string(),
            "1".to_string(),
            "NULL".to_string(),
            "NULL".to_string(),
            created_at.to_rfc3339(),
        ];

        let rows = wit_bindings::sql_state::query(sql, &params)
            .map_err(BranchRepositoryError::SqlError)?;

        let row = rows.first().ok_or_else(|| {
            BranchRepositoryError::SqlError("INSERT failed to return any rows".to_string())
        })?;

        let id_val = if let Some(idx) = row.columns.iter().position(|c| c == "id") {
            row.values.get(idx).ok_or_else(|| {
                BranchRepositoryError::ParseError("Returned row missing id value".to_string())
            })?
        } else {
            row.values.first().ok_or_else(|| {
                BranchRepositoryError::ParseError("Returned row has no values".to_string())
            })?
        };

        let uuid = uuid::Uuid::parse_str(id_val)?;
        Ok(MergeId::from_uuid(uuid))
    }

    fn approve_merge(
        &self,
        merge_id: MergeId,
        approver: &str,
    ) -> Result<(), BranchRepositoryError> {
        let sql = "UPDATE merge_queue SET status = ?, approved_by = ?, approved_at = ? \
                   WHERE id = ? AND status = 'pending'";

        let approved_at = Utc::now();

        let params = vec![
            "approved".to_string(),
            approver.to_string(),
            approved_at.to_rfc3339(),
            merge_id.to_string(),
        ];

        let affected = wit_bindings::sql_state::execute(sql, &params)
            .map_err(BranchRepositoryError::SqlError)?;

        if affected == 0 {
            return Err(BranchRepositoryError::MergeRequestNotFound(merge_id));
        }

        Ok(())
    }

    /// Retrieves a merge request from the database by its ID.
    ///
    /// # Preconditions
    /// - `merge_id` must be a valid UUID format
    ///
    /// # Postconditions
    /// - Returns `Ok(Some(merge_request))` if found
    /// - Returns `Ok(None)` if not found
    /// - Returns `Err(BranchRepositoryError::SqlError)` on database failure
    fn get_merge_request(
        &self,
        merge_id: MergeId,
    ) -> Result<Option<MergeRequest>, BranchRepositoryError> {
        let sql = "SELECT id, branch_id, parent_id, strategy, status, requires_approval, approved_by, approved_at, created_at, staging_session_id, started_at, completed_at FROM merge_queue WHERE id = ?";

        let params = vec![merge_id.to_string()];

        let rows = wit_bindings::sql_state::query(sql, &params)
            .map_err(BranchRepositoryError::SqlError)?;

        if rows.is_empty() {
            return Ok(None);
        }

        let row = &rows[0];
        Ok(Some(Self::parse_merge_request_row(
            &row.columns,
            &row.values,
        )?))
    }

    /// Updates an existing merge request in the database.
    ///
    /// # Preconditions
    /// - `merge_request.id()` must exist in the database
    ///
    /// # Postconditions
    /// - Returns `Ok(())` on successful update
    /// - Returns `Err(BranchRepositoryError::MergeRequestNotFound)` if merge request doesn't exist
    /// - Returns `Err(BranchRepositoryError::SqlError)` on database failure
    fn update_merge_request(
        &self,
        merge_request: &MergeRequest,
    ) -> Result<(), BranchRepositoryError> {
        let sql = "UPDATE merge_queue SET status = ?, approved_by = ?, approved_at = ?, staging_session_id = ?, started_at = ?, completed_at = ? WHERE id = ?";

        let approved_at = merge_request
            .approved_at()
            .and_then(|ts| chrono::DateTime::from_timestamp(ts, 0))
            .map_or_else(|| "NULL".to_string(), |dt| dt.to_rfc3339());

        let started_at = merge_request
            .started_at()
            .and_then(|ts| chrono::DateTime::from_timestamp(ts, 0))
            .map_or_else(|| "NULL".to_string(), |dt| dt.to_rfc3339());

        let completed_at = merge_request
            .completed_at()
            .and_then(|ts| chrono::DateTime::from_timestamp(ts, 0))
            .map_or_else(|| "NULL".to_string(), |dt| dt.to_rfc3339());

        let staging_session_id = merge_request
            .staging_session_id()
            .map_or_else(|| "NULL".to_string(), std::string::ToString::to_string);

        let approved_by = merge_request
            .approved_by()
            .map_or_else(|| "NULL".to_string(), std::string::ToString::to_string);

        let params = vec![
            format!("{:?}", merge_request.status()).to_lowercase(),
            approved_by,
            approved_at,
            staging_session_id,
            started_at,
            completed_at,
            merge_request.id().to_string(),
        ];

        let affected = wit_bindings::sql_state::execute(sql, &params)
            .map_err(BranchRepositoryError::SqlError)?;

        Self::expect_merge_affected(merge_request.id(), affected)
    }
}

impl Transactional for WitBranchRepository {
    fn with_transaction<F, T, E>(&self, operations: F) -> Result<T, E>
    where
        F: FnOnce(&mut Transaction) -> Result<T, E>,
        E: From<TransactionError>,
    {
        let mut tx = Transaction::begin()?;

        match operations(&mut tx) {
            Ok(result) => {
                tx.commit()?;
                Ok(result)
            }
            Err(e) => {
                let _ = tx.rollback();
                Err(e)
            }
        }
    }
}
