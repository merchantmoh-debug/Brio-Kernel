use super::*;
use crate::store::policy::PrefixPolicy;
use anyhow::Result;
use sqlx::sqlite::SqlitePoolOptions;

async fn setup_store() -> Result<(SqlStore, sqlx::SqlitePool)> {
    let pool = SqlitePoolOptions::new().connect("sqlite::memory:").await?;

    let store = SqlStore::new(pool.clone(), Box::new(PrefixPolicy));

    sqlx::query("CREATE TABLE agent_1_data (id INTEGER PRIMARY KEY, content TEXT)")
        .execute(&pool)
        .await?;

    Ok((store, pool))
}

#[tokio::test]
async fn test_store_query_success() -> Result<()> {
    let (store, _pool) = setup_store().await?;

    store
        .execute(
            "agent_1",
            "INSERT INTO agent_1_data (content) VALUES (?)",
            vec!["hello".to_string()],
        )
        .await?;

    let rows = store
        .query("agent_1", "SELECT * FROM agent_1_data", vec![])
        .await?;

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].values[1], "hello");

    Ok(())
}

#[tokio::test]
async fn test_store_policy_violation() -> Result<()> {
    let (store, _) = setup_store().await?;

    let result = store
        .query("agent_2", "SELECT * FROM agent_1_data", vec![])
        .await;

    assert!(result.is_err());
    match result.unwrap_err() {
        StoreError::PolicyError(_) => {}
        err => panic!("Unexpected error: {err:?}"),
    }

    Ok(())
}

#[tokio::test]
async fn test_store_generic_types() -> Result<()> {
    let (store, _) = setup_store().await?;

    store
        .execute(
            "agent_1",
            "INSERT INTO agent_1_data (id, content) VALUES (99, 'test')",
            vec![],
        )
        .await?;

    let rows = store
        .query(
            "agent_1",
            "SELECT id, content FROM agent_1_data WHERE id = 99",
            vec![],
        )
        .await?;

    assert_eq!(rows[0].values[0], "99");
    assert_eq!(rows[0].values[1], "test");

    Ok(())
}
