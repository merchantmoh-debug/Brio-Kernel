//! Shared test utilities for integration tests.
//!
//! Provides common setup, teardown, and helper functions for testing
//! tool components, mesh communication, VFS operations, and branch management.

#![allow(
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_possible_truncation
)]

use anyhow::Result;
use brio_kernel::host::BrioHostState;
use brio_kernel::inference::{ChatRequest, ChatResponse, InferenceError, LLMProvider};
use brio_kernel::infrastructure::config::SandboxSettings;
use brio_kernel::mesh::{MeshMessage, Payload};
use sqlx::Row;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use supervisor::domain::{AgentId, Priority, Task, TaskId, TaskStatus};
use supervisor::mesh_client::{AgentDispatcher, DispatchResult, MeshError};
use supervisor::orchestrator::{Planner, PlannerError, Supervisor};
use supervisor::repository::{RepositoryError, TaskRepository};
use supervisor::selector::KeywordAgentSelector;
use tempfile::TempDir;
use tokio::sync::mpsc;

/// Integration test context providing shared resources.
pub struct IntegrationTestContext {
    /// Temporary directory for test files
    pub temp_dir: TempDir,
    /// Host state for kernel operations
    pub host: Arc<BrioHostState>,
    /// Agent message receiver (if agent registered)
    pub agent_msg_rx: Option<mpsc::Receiver<MeshMessage>>,
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
            agent_msg_rx: None,
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
            agent_msg_rx: None,
        })
    }

    /// Gets the path to the temporary directory.
    pub fn temp_path(&self) -> PathBuf {
        self.temp_dir.path().to_path_buf()
    }

    /// Creates a subdirectory in the temp directory.
    pub fn create_subdir(&self, name: &str) -> Result<PathBuf> {
        let path = self.temp_dir.path().join(name);
        std::fs::create_dir_all(&path)?;
        Ok(path)
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
        self.agent_msg_rx = Some(rx);
        // Return the receiver and take it from self
        self.agent_msg_rx.take().unwrap()
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

/// Mock planner for testing.
pub struct MockPlanner;

impl Planner for MockPlanner {
    fn plan(&self, _objective: &str) -> Result<Option<Vec<String>>, PlannerError> {
        Ok(None)
    }
}

/// Test task repository implementation.
#[derive(Clone)]
pub struct TestTaskRepository {
    host: Arc<BrioHostState>,
}

impl TestTaskRepository {
    pub fn new(host: Arc<BrioHostState>) -> Self {
        Self { host }
    }
}

impl TaskRepository for TestTaskRepository {
    fn fetch_active_tasks(&self) -> Result<Vec<Task>, RepositoryError> {
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                let rows = sqlx::query(
                    "SELECT * FROM tasks WHERE status IN ('pending', 'planning', 'executing', 'verifying') ORDER BY priority DESC",
                )
                .fetch_all(self.host.db())
                .await
                .map_err(|e| RepositoryError::SqlError(e.to_string()))?;

                rows.into_iter()
                    .map(|r| {
                        let id: i64 = r.try_get("id").map_err(|e| RepositoryError::SqlError(e.to_string()))?;
                        let content: String = r.try_get("content").map_err(|e| RepositoryError::SqlError(e.to_string()))?;
                        let priority: i64 = r.try_get("priority").map_err(|e| RepositoryError::SqlError(e.to_string()))?;
                        let status_str: String = r.try_get("status").map_err(|e| RepositoryError::SqlError(e.to_string()))?;
                        let status = TaskStatus::parse(&status_str).map_err(|e| RepositoryError::ParseError(e.to_string()))?;

                        let assigned_agent_str: Option<String> = r.try_get("assigned_agent").unwrap_or(None);
                        let assigned_agent = assigned_agent_str.map(AgentId::new).transpose().map_err(|e| RepositoryError::ParseError(e.to_string()))?;

                        Task::new(
                            TaskId::new(id as u64),
                            content,
                            Priority::new(priority as u8),
                            status,
                            None,
                            assigned_agent,
                            HashSet::new(),
                        ).map_err(|e| RepositoryError::ParseError(e.to_string()))
                    })
                    .collect()
            })
        })
    }

    fn update_status(&self, task_id: TaskId, status: TaskStatus) -> Result<(), RepositoryError> {
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                sqlx::query("UPDATE tasks SET status = ? WHERE id = ?")
                    .bind(status.as_str())
                    .bind(task_id.inner() as i64)
                    .execute(self.host.db())
                    .await
                    .map_err(|e| RepositoryError::SqlError(e.to_string()))?;
                Ok(())
            })
        })
    }

    fn assign_agent(&self, task_id: TaskId, agent: &AgentId) -> Result<(), RepositoryError> {
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                sqlx::query("UPDATE tasks SET assigned_agent = ? WHERE id = ?")
                    .bind(agent.as_str())
                    .bind(task_id.inner() as i64)
                    .execute(self.host.db())
                    .await
                    .map_err(|e| RepositoryError::SqlError(e.to_string()))?;
                Ok(())
            })
        })
    }

    fn mark_assigned(&self, task_id: TaskId, agent: &AgentId) -> Result<(), RepositoryError> {
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                sqlx::query(
                    "UPDATE tasks SET status = 'assigned', assigned_agent = ? WHERE id = ?",
                )
                .bind(agent.as_str())
                .bind(task_id.inner() as i64)
                .execute(self.host.db())
                .await
                .map_err(|e| RepositoryError::SqlError(e.to_string()))?;
                Ok(())
            })
        })
    }

    fn mark_completed(&self, task_id: TaskId) -> Result<(), RepositoryError> {
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                sqlx::query("UPDATE tasks SET status = 'completed' WHERE id = ?")
                    .bind(task_id.inner() as i64)
                    .execute(self.host.db())
                    .await
                    .map_err(|e| RepositoryError::SqlError(e.to_string()))?;
                Ok(())
            })
        })
    }

    fn mark_failed(&self, task_id: TaskId, reason: &str) -> Result<(), RepositoryError> {
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                sqlx::query("UPDATE tasks SET status = 'failed', failure_reason = ? WHERE id = ?")
                    .bind(reason)
                    .bind(task_id.inner() as i64)
                    .execute(self.host.db())
                    .await
                    .map_err(|e| RepositoryError::SqlError(e.to_string()))?;
                Ok(())
            })
        })
    }

    fn create_task(
        &self,
        content: String,
        priority: Priority,
        parent_id: Option<TaskId>,
    ) -> Result<TaskId, RepositoryError> {
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                let id = sqlx::query(
                    "INSERT INTO tasks (content, priority, status, parent_id) VALUES (?, ?, ?, ?) RETURNING id",
                )
                .bind(content)
                .bind(i64::from(priority.inner()))
                .bind(TaskStatus::Pending.as_str())
                .bind(parent_id.map(|id| id.inner() as i64))
                .fetch_one(self.host.db())
                .await
                .map_err(|e| RepositoryError::SqlError(e.to_string()))?
                .get::<i64, _>("id");

                Ok(TaskId::new(id as u64))
            })
        })
    }

    fn fetch_subtasks(&self, parent_id: TaskId) -> Result<Vec<Task>, RepositoryError> {
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                let rows =
                    sqlx::query("SELECT * FROM tasks WHERE parent_id = ? ORDER BY priority DESC")
                        .bind(parent_id.inner() as i64)
                        .fetch_all(self.host.db())
                        .await
                        .map_err(|e| RepositoryError::SqlError(e.to_string()))?;

                rows.into_iter()
                    .map(|r| {
                        let id: i64 = r
                            .try_get("id")
                            .map_err(|e| RepositoryError::SqlError(e.to_string()))?;
                        let content: String = r
                            .try_get("content")
                            .map_err(|e| RepositoryError::SqlError(e.to_string()))?;
                        let priority: i64 = r
                            .try_get("priority")
                            .map_err(|e| RepositoryError::SqlError(e.to_string()))?;
                        let status_str: String = r
                            .try_get("status")
                            .map_err(|e| RepositoryError::SqlError(e.to_string()))?;
                        let status = TaskStatus::parse(&status_str)
                            .map_err(|e| RepositoryError::ParseError(e.to_string()))?;

                        let assigned_agent_str: Option<String> =
                            r.try_get("assigned_agent").unwrap_or(None);
                        let assigned_agent = assigned_agent_str
                            .map(AgentId::new)
                            .transpose()
                            .map_err(|e| RepositoryError::ParseError(e.to_string()))?;

                        let parent_id_val: Option<i64> = r.try_get("parent_id").unwrap_or(None);
                        let parent_id = parent_id_val.map(|v| TaskId::new(v as u64));

                        Task::new(
                            TaskId::new(id as u64),
                            content,
                            Priority::new(priority as u8),
                            status,
                            parent_id,
                            assigned_agent,
                            HashSet::new(),
                        )
                        .map_err(|e| RepositoryError::ParseError(e.to_string()))
                    })
                    .collect()
            })
        })
    }
}

/// Test agent dispatcher implementation.
pub struct TestDispatcher {
    host: Arc<BrioHostState>,
}

impl TestDispatcher {
    pub fn new(host: Arc<BrioHostState>) -> Self {
        Self { host }
    }
}

impl AgentDispatcher for TestDispatcher {
    fn dispatch(&self, agent: &AgentId, task: &Task) -> Result<DispatchResult, MeshError> {
        use brio_kernel::host::MeshHandler;

        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                MeshHandler::mesh_call(
                    &*self.host,
                    agent.as_str(),
                    "execute",
                    Payload::Json(Box::new(task.content().to_string())),
                )
                .await
            })
        })
        .map(|_| DispatchResult::Accepted)
        .map_err(|e: anyhow::Error| MeshError::TransportError(e.to_string()))
    }
}

/// Creates a simple test supervisor.
pub fn create_test_supervisor(
    host: Arc<BrioHostState>,
) -> Supervisor<TestTaskRepository, TestDispatcher, MockPlanner, KeywordAgentSelector> {
    let repo = TestTaskRepository::new(host.clone());
    let dispatcher = TestDispatcher::new(host);
    let planner = MockPlanner;
    let selector = KeywordAgentSelector;
    Supervisor::new(repo, dispatcher, planner, selector)
}

/// Runs a supervisor cycle and returns the number of tasks processed.
pub async fn run_supervisor_cycle(host: Arc<BrioHostState>) -> Result<u32> {
    tokio::task::spawn_blocking(move || {
        let supervisor = create_test_supervisor(host);
        let mut total = 0;
        loop {
            let n = supervisor.poll_tasks().expect("Supervisor poll failed");
            if n == 0 {
                break;
            }
            total += n;
        }
        Ok::<u32, anyhow::Error>(total)
    })
    .await?
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
