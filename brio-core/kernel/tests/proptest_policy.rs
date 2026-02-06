//! Property-based tests for SQL policy validation.
//!
//! Uses proptest to verify that the `PrefixPolicy` correctly allows or denies
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
        "[a-z]{4,8}".prop_map(|s| format!("{s}_table")),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    /// Property: Any SELECT query on a table prefixed with `{scope}_` should be allowed.
    #[test]
    fn select_on_matching_prefix_always_allowed(
        scope in scope_strategy()
    ) {
        let table = format!("{scope}_data");
        let sql = format!("SELECT * FROM {table}");
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
        if bad_table.starts_with(&format!("{scope}_")) {
            return Ok(());
        }

        let sql = format!("SELECT * FROM {bad_table}");
        let policy = PrefixPolicy;

        prop_assert!(policy.authorize(&scope, &sql).is_err());
    }

    /// Property: INSERT on matching table is allowed.
    #[test]
    fn insert_on_matching_prefix_allowed(
        scope in scope_strategy()
    ) {
        let table = format!("{scope}_records");
        let sql = format!("INSERT INTO {table} (name) VALUES ('test')");
        let policy = PrefixPolicy;

        prop_assert!(policy.authorize(&scope, &sql).is_ok());
    }

    /// Property: UPDATE on matching table is allowed.
    #[test]
    fn update_on_matching_prefix_allowed(
        scope in scope_strategy()
    ) {
        let table = format!("{scope}_items");
        let sql = format!("UPDATE {table} SET status = 'done' WHERE id = 1");
        let policy = PrefixPolicy;

        prop_assert!(policy.authorize(&scope, &sql).is_ok());
    }

    /// Property: DELETE on matching table is allowed.
    #[test]
    fn delete_on_matching_prefix_allowed(
        scope in scope_strategy()
    ) {
        let table = format!("{scope}_logs");
        let sql = format!("DELETE FROM {table} WHERE id > 100");
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
        if bad_table.starts_with(&format!("{scope}_")) {
            return Ok(());
        }

        let good_table = format!("{scope}_data");
        let sql = format!(
            "SELECT * FROM {good_table} JOIN {bad_table} ON {good_table}.id = {bad_table}.id"
        );
        let policy = PrefixPolicy;

        prop_assert!(policy.authorize(&scope, &sql).is_err());
    }

    /// Property: JOIN between two matching tables should succeed.
    #[test]
    fn join_between_matching_tables_allowed(
        scope in scope_strategy()
    ) {
        let table1 = format!("{scope}_orders");
        let table2 = format!("{scope}_items");
        let sql = format!(
            "SELECT * FROM {table1} JOIN {table2} ON {table1}.order_id = {table2}.id"
        );
        let policy = PrefixPolicy;

        prop_assert!(policy.authorize(&scope, &sql).is_ok());
    }

    /// Property: Subqueries with non-matching tables should be denied.
    #[test]
    fn subquery_with_non_matching_table_denied(
        scope in scope_strategy()
    ) {
        let good_table = format!("{scope}_data");
        let sql = format!(
            "SELECT * FROM {good_table} WHERE id IN (SELECT id FROM system_admin)"
        );
        let policy = PrefixPolicy;

        prop_assert!(policy.authorize(&scope, &sql).is_err());
    }
}
