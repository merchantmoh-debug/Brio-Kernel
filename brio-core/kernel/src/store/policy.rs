use sqlparser::{
    ast::{TableFactor, Visit, Visitor},
    dialect::GenericDialect,
    parser::Parser,
};
use std::ops::ControlFlow;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PolicyError {
    #[error("SQL Parse Error: {0}")]
    ParseError(String),
    #[error("Access Denied: Table '{0}' does not match scope '{1}'")]
    ScopeViolation(String, String),
    #[error("Policy Violation: {0}")]
    Violation(String),
}

/// Defines the authorization contract for SQL execution.
pub trait QueryPolicy: Send + Sync {
    /// Verify if the given SQL is allowed for the given scope.
    ///
    /// # Errors
    ///
    /// Returns an error if the SQL parsing fails or if the query violates the policy.
    fn authorize(&self, scope: &str, sql: &str) -> Result<(), PolicyError>;
}

/// A strict policy that ensures all accessed tables start with `{scope}_`.
pub struct PrefixPolicy;

impl QueryPolicy for PrefixPolicy {
    fn authorize(&self, scope: &str, sql: &str) -> Result<(), PolicyError> {
        let dialect = GenericDialect {};
        let ast =
            Parser::parse_sql(&dialect, sql).map_err(|e| PolicyError::ParseError(e.to_string()))?;

        for statement in ast {
            // We use a Visitor to traverse the AST and find all table references.
            let mut visitor = TableVisitor { scope };
            if let ControlFlow::Break(err) = statement.visit(&mut visitor) {
                return Err(err);
            }
        }

        Ok(())
    }
}

struct TableVisitor<'a> {
    scope: &'a str,
}

impl Visitor for TableVisitor<'_> {
    type Break = PolicyError;

    fn pre_visit_table_factor(&mut self, table_factor: &TableFactor) -> ControlFlow<Self::Break> {
        if let TableFactor::Table { name, .. } = table_factor
            && let Some(table_part) = name.0.last()
            && let Some(ident) = table_part.as_ident()
        {
            let table_name = ident.value.as_str();
            let expected_prefix = format!("{}_", self.scope);

            if !table_name.starts_with(&expected_prefix) {
                return ControlFlow::Break(PolicyError::ScopeViolation(
                    table_name.to_string(),
                    self.scope.to_string(),
                ));
            }
        }
        ControlFlow::Continue(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_scope_query() {
        let policy = PrefixPolicy;
        let sql = "SELECT * FROM agent_1_data WHERE id = 1";
        assert!(policy.authorize("agent_1", sql).is_ok());
    }

    #[test]
    fn test_invalid_scope_query() {
        let policy = PrefixPolicy;
        let sql = "SELECT * FROM system_config";
        assert!(policy.authorize("agent_1", sql).is_err());
    }

    #[test]
    fn test_join_scope_violation() {
        let policy = PrefixPolicy;
        let sql = "SELECT * FROM agent_1_data JOIN system_users ON agent_1_data.user_id = system_users.id";
        assert!(policy.authorize("agent_1", sql).is_err());
    }

    #[test]
    fn test_quoted_identifiers() {
        let policy = PrefixPolicy;
        // In SQL, "agent_1_data" is a valid identifier. sqlparser handles quotes.
        let sql = "SELECT * FROM \"agent_1_data\"";
        assert!(policy.authorize("agent_1", sql).is_ok());
    }

    #[test]
    fn test_drop_table_valid() {
        let policy = PrefixPolicy;
        let sql = "DROP TABLE agent_1_temp";
        assert!(policy.authorize("agent_1", sql).is_ok());
    }
}
