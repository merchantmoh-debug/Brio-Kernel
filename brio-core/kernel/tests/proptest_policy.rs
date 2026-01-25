//! Property-based tests for SQL policy validation.
//!
//! Uses proptest to verify that the PrefixPolicy correctly allows or denies
//! queries based on table name prefixes.

use brio_kernel::store::policy::{PrefixPolicy, QueryPolicy};
use proptest::prelude::*;

/// Strategy for generating valid scope identifiers
fn scope_strategy() -> impl Strategy<Value = String> {
    "[a-z][a-z0-9]{0,7}".prop_filter("Valid scope", |s| !s.is_empty() && s.len() <= 8)
}

/// Strategy for generating table names that don't match the scope
fn non_matching_table_strategy() -> impl Strategy<Value = String> {
    prop_oneof![
        Just("system_config".to_string()),
        Just("admin_users".to_string()),
        Just("public_data".to_string()),
        "[a-z]{4,8}".prop_map(|s| format!("{}_table", s)),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    /// Property: Any SELECT query on a table prefixed with `{scope}_` should be allowed.
    #[test]
    fn select_on_matching_prefix_always_allowed(
        scope in scope_strategy()
    ) {
        let table = format!("{}_data", scope);
        let sql = format!("SELECT * FROM {}", table);
        let policy = PrefixPolicy;

        prop_assert!(policy.authorize(&scope, &sql).is_ok());
    }

    /// Property: Any SELECT query on a table NOT prefixed with `{scope}_` should be denied.
    #[test]
    fn select_on_non_matching_prefix_always_denied(
        scope in scope_strategy(),
        bad_table in non_matching_table_strategy()
    ) {
        // Skip if table accidentally matches the scope
        if bad_table.starts_with(&format!("{}_", scope)) {
            return Ok(());
        }

        let sql = format!("SELECT * FROM {}", bad_table);
        let policy = PrefixPolicy;

        prop_assert!(policy.authorize(&scope, &sql).is_err());
    }

    /// Property: INSERT on matching table is allowed.
    #[test]
    fn insert_on_matching_prefix_allowed(
        scope in scope_strategy()
    ) {
        let table = format!("{}_records", scope);
        let sql = format!("INSERT INTO {} (name) VALUES ('test')", table);
        let policy = PrefixPolicy;

        prop_assert!(policy.authorize(&scope, &sql).is_ok());
    }

    /// Property: UPDATE on matching table is allowed.
    #[test]
    fn update_on_matching_prefix_allowed(
        scope in scope_strategy()
    ) {
        let table = format!("{}_items", scope);
        let sql = format!("UPDATE {} SET status = 'done' WHERE id = 1", table);
        let policy = PrefixPolicy;

        prop_assert!(policy.authorize(&scope, &sql).is_ok());
    }

    /// Property: DELETE on matching table is allowed.
    #[test]
    fn delete_on_matching_prefix_allowed(
        scope in scope_strategy()
    ) {
        let table = format!("{}_logs", scope);
        let sql = format!("DELETE FROM {} WHERE id > 100", table);
        let policy = PrefixPolicy;

        prop_assert!(policy.authorize(&scope, &sql).is_ok());
    }

    /// Property: JOIN with one matching and one non-matching table should fail.
    #[test]
    fn join_with_non_matching_table_denied(
        scope in scope_strategy(),
        bad_table in non_matching_table_strategy()
    ) {
        // Skip if bad_table accidentally matches
        if bad_table.starts_with(&format!("{}_", scope)) {
            return Ok(());
        }

        let good_table = format!("{}_data", scope);
        let sql = format!(
            "SELECT * FROM {} JOIN {} ON {}.id = {}.id",
            good_table, bad_table, good_table, bad_table
        );
        let policy = PrefixPolicy;

        prop_assert!(policy.authorize(&scope, &sql).is_err());
    }

    /// Property: JOIN between two matching tables should succeed.
    #[test]
    fn join_between_matching_tables_allowed(
        scope in scope_strategy()
    ) {
        let table1 = format!("{}_orders", scope);
        let table2 = format!("{}_items", scope);
        let sql = format!(
            "SELECT * FROM {} JOIN {} ON {}.order_id = {}.id",
            table1, table2, table1, table2
        );
        let policy = PrefixPolicy;

        prop_assert!(policy.authorize(&scope, &sql).is_ok());
    }

    /// Property: Subqueries with non-matching tables should be denied.
    #[test]
    fn subquery_with_non_matching_table_denied(
        scope in scope_strategy()
    ) {
        let good_table = format!("{}_data", scope);
        let sql = format!(
            "SELECT * FROM {} WHERE id IN (SELECT id FROM system_admin)",
            good_table
        );
        let policy = PrefixPolicy;

        prop_assert!(policy.authorize(&scope, &sql).is_err());
    }
}
