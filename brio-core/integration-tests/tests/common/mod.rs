//! Shared test utilities for integration tests.
//!
//! Provides common setup, teardown, and helper functions for testing
//! tool components, mesh communication, VFS operations, and branch management.

// Allow dead code since this is a shared test utilities module
// and different tests may use different parts
#![allow(dead_code)]
#![allow(
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_possible_truncation
)]

use anyhow::Result;
use brio_kernel::host::BrioHostState;
use brio_kernel::inference::{ChatRequest, ChatResponse, InferenceError, LLMProvider};
use brio_kernel::infrastructure::config::SandboxSettings;
use brio_kernel::mesh::MeshMessage;
use sqlx::Row;
use std::path::PathBuf;
use std::sync::Arc;
use tempfile::TempDir;
use tokio::sync::mpsc;

/// Integration test context providing shared resources.
pub struct IntegrationTestContext {
    /// Temporary directory for test files (kept alive for test duration)
    #[allow(dead_code)]
    pub temp_dir: TempDir,
    /// Host state for kernel operations
    pub host: Arc<BrioHostState>,
}

impl IntegrationTestContext {
    /// Creates a new test context with mock provider.
    pub async fn new() -> Result<Self> {
        let temp_dir = TempDir::new()?;
        let host = Arc::new(
            BrioHostState::with_provider("sqlite::memory:", Box::new(MockProvider)).await?,
        );

        Ok(Self {
            temp_dir,
            host,
        })
    }

    /// Creates a test context with custom sandbox settings.
    pub async fn with_sandbox(sandbox: SandboxSettings) -> Result<Self> {
        let temp_dir = TempDir::new()?;
        let registry = brio_kernel::inference::ProviderRegistry::new();
        registry.register_arc("default", Arc::new(MockProvider));
        registry.set_default("default");

        let host = Arc::new(BrioHostState::new("sqlite::memory:", registry, None, sandbox).await?);

        Ok(Self {
            temp_dir,
            host,
        })
    }

    /// Gets the path to the temporary directory.
    pub fn temp_path(&self) -> PathBuf {
        self.temp_dir.path().to_path_buf()
    }

    /// Creates a test file with given content.
    pub fn create_test_file(&self, name: &str, content: &str) -> Result<PathBuf> {
        let path = self.temp_dir.path().join(name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, content)?;
        Ok(path)
    }

    /// Registers an agent and returns the receiver for its messages.
    pub fn register_agent(&mut self, id: &str) -> mpsc::Receiver<MeshMessage> {
        let (tx, rx) = mpsc::channel(10);
        self.host.register_component(id.to_string(), tx);
        rx
    }

    /// Gets the database pool for raw queries.
    pub fn db(&self) -> &sqlx::SqlitePool {
        self.host.db()
    }
}

// =============================================================================
// Mocks
// =============================================================================

/// Mock LLM provider for testing.
pub struct MockProvider;

#[async_trait::async_trait]
impl LLMProvider for MockProvider {
    async fn chat(&self, _request: ChatRequest) -> Result<ChatResponse, InferenceError> {
        Ok(ChatResponse {
            content: "Mock response".to_string(),
            usage: None,
        })
    }
}

/// Initializes the tasks table in the database.
pub async fn init_tasks_table(host: &BrioHostState) -> Result<()> {
    sqlx::query(
        r"
        CREATE TABLE IF NOT EXISTS tasks (
            id INTEGER PRIMARY KEY,
            content TEXT NOT NULL,
            priority INTEGER NOT NULL,
            status TEXT NOT NULL,
            assigned_agent TEXT,
            failure_reason TEXT,
            parent_id INTEGER
        );
        ",
    )
    .execute(host.db())
    .await?;
    Ok(())
}

/// Creates a test task in the database.
pub async fn create_test_task(
    host: &BrioHostState,
    content: &str,
    priority: i32,
    status: &str,
) -> Result<i64> {
    let id =
        sqlx::query("INSERT INTO tasks (content, priority, status) VALUES (?, ?, ?) RETURNING id")
            .bind(content)
            .bind(priority)
            .bind(status)
            .fetch_one(host.db())
            .await?
            .get::<i64, _>("id");
    Ok(id)
}
