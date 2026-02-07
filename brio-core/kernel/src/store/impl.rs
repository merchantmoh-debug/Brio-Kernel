//! SQL store implementation with policy enforcement.
//!
//! This module provides a SQLite-backed store with configurable query policies
//! for secure data access and isolation between different scopes.

use anyhow::Result;
use sqlx::{
    Column, Row, TypeInfo, ValueRef,
    sqlite::{SqlitePool, SqliteRow},
};
use tracing::instrument;

use crate::store::policy::{PolicyError, QueryPolicy};

/// Errors that can occur when using the store.
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    /// Database-related error.
    #[error("Database Error: {0}")]
    DbError(#[from] sqlx::Error),
    /// Policy violation error.
    #[error("Policy Violation: {0}")]
    PolicyError(#[from] PolicyError),
    /// Internal error.
    #[error("Internal Error: {0}")]
    Internal(#[from] anyhow::Error),
}

/// A generic Row representation matching the WIT definition.
#[derive(Debug, Clone)]
pub struct GenericRow {
    /// Column names.
    pub columns: Vec<String>,
    /// Row values.
    pub values: Vec<String>,
}

/// SQL store with policy enforcement.
pub struct SqlStore {
    pool: SqlitePool,
    policy: Box<dyn QueryPolicy>,
}

impl SqlStore {
    /// Creates a new SQL store.
    ///
    /// # Arguments
    ///
    /// * `pool` - The `SQLite` connection pool.
    /// * `policy` - The query policy to enforce.
    #[must_use]
    pub fn new(pool: SqlitePool, policy: Box<dyn QueryPolicy>) -> Self {
        Self { pool, policy }
    }

    /// Execute a query that returns rows (SELECT).
    /// Enforces policy before execution.
    ///
    /// # Errors
    ///
    /// Returns an error if the policy check fails or if the query execution fails.
    #[instrument(skip(self, sql), fields(scope = %scope))]
    pub async fn query(
        &self,
        scope: &str,
        sql: &str,
        params: Vec<String>,
    ) -> Result<Vec<GenericRow>, StoreError> {
        // 1. Enforce Policy
        self.policy.authorize(scope, sql)?;

        // 2. Prepare Query
        let mut query_builder = sqlx::query(sql);
        for param in params {
            query_builder = query_builder.bind(param);
        }

        // 3. Execute
        let rows: Vec<SqliteRow> = query_builder.fetch_all(&self.pool).await?;

        // 4. Map Results
        let mut results = Vec::new();
        for row in rows {
            let columns: Vec<String> = row.columns().iter().map(|c| c.name().to_string()).collect();
            let mut values = Vec::new();
            for (i, col) in row.columns().iter().enumerate() {
                values.push(convert_cell(&row, i, col));
            }
            results.push(GenericRow { columns, values });
        }

        Ok(results)
    }

    /// Execute a statement that modifies state (INSERT, UPDATE, DELETE).
    /// Enforces policy before execution.
    ///
    /// # Errors
    ///
    /// Returns an error if the policy check fails or if the statement execution fails.
    #[instrument(skip(self, sql), fields(scope = %scope))]
    pub async fn execute(
        &self,
        scope: &str,
        sql: &str,
        params: Vec<String>,
    ) -> Result<u32, StoreError> {
        // 1. Enforce Policy
        self.policy.authorize(scope, sql)?;

        // 2. Prepare Query
        let mut query_builder = sqlx::query(sql);
        for param in params {
            query_builder = query_builder.bind(param);
        }

        // 3. Execute
        let result = query_builder.execute(&self.pool).await?;

        Ok(u32::try_from(result.rows_affected()).unwrap_or(u32::MAX))
    }
}

/// Helper to convert a single cell to string using best-effort strategy.
/// This encapsulates the type erasure logic.
fn convert_cell(row: &SqliteRow, index: usize, col: &sqlx::sqlite::SqliteColumn) -> String {
    if row.try_get_raw(index).map(|r| r.is_null()).unwrap_or(true) {
        return "NULL".to_string();
    }

    let type_info = col.type_info();
    if type_info.is_null() {
        return "NULL".to_string();
    }

    // Attempt String conversion
    if let Ok(s) = row.try_get::<String, _>(index) {
        return s;
    }

    // Attempt Integer conversion
    if let Ok(n) = row.try_get::<i64, _>(index) {
        return n.to_string();
    }

    // Attempt Float conversion
    if let Ok(f) = row.try_get::<f64, _>(index) {
        return f.to_string();
    }

    "UNSUPPORTED_TYPE".to_string()
}
