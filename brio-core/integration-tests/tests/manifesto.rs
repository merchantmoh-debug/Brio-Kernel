//! Integration tests for the supervisor manifesto scenario.
//!
//! Tests end-to-end agent orchestration with real and mock AI providers,
//! including task lifecycle management and distributed mesh communication.

// Allow casting warnings in tests - these are typically safe conversions for test data
// where we control the input values and ranges. These casts are necessary for
// database ID conversions in the test repository implementations.
#![allow(
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_possible_truncation
)]

use anyhow::Result;
use brio_kernel::host::BrioHostState;
use brio_kernel::inference::{ChatRequest, ChatResponse, InferenceError, LLMProvider};
use brio_kernel::mesh::{MeshMessage, Payload};
use sqlx::Row;
use std::collections::HashSet;
use std::sync::Arc;
use supervisor::domain::{AgentId, Priority, Task, TaskId, TaskStatus};
use supervisor::mesh_client::{AgentDispatcher, DispatchResult, MeshError};
use supervisor::orchestrator::{Planner, PlannerError, Supervisor};
use supervisor::repository::{RepositoryError, TaskRepository};
use tokio::sync::mpsc;

// =============================================================================
// Fixtures (Encapsulation)
// =============================================================================

struct TestEnvironment {
    host: Arc<BrioHostState>,
    root: std::path::PathBuf,
    agent_msg_rx: Option<mpsc::Receiver<MeshMessage>>,
}

impl TestEnvironment {
    async fn setup_with_provider(provider: Box<dyn LLMProvider>) -> Result<Self> {
        let root = std::env::temp_dir().join("brio_manifesto_test");
        if root.exists() {
            std::fs::remove_dir_all(&root)?;
        }
        std::fs::create_dir_all(&root)?;

        let host = Arc::new(BrioHostState::with_provider("sqlite::memory:", provider).await?);

        let env = Self {
            host,
            root,
            agent_msg_rx: None,
        };
        env.init_db().await?;
        Ok(env)
    }

    async fn setup() -> Result<Self> {
        Self::setup_with_provider(Box::new(MockProvider)).await
    }

    async fn init_db(&self) -> Result<()> {
        sqlx::query(
            r"
            CREATE TABLE tasks (
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
        .execute(self.host.db())
        .await?;
        Ok(())
    }

    fn register_agent(&mut self, id: &str) {
        let (tx, rx) = mpsc::channel(10);
        self.host.register_component(id.to_string(), tx);
        self.agent_msg_rx = Some(rx);
    }
}

// =============================================================================
// Mocks (Single Responsibility)
// =============================================================================

struct MockProvider;
#[async_trait::async_trait]
impl LLMProvider for MockProvider {
    async fn chat(&self, _request: ChatRequest) -> Result<ChatResponse, InferenceError> {
        Ok(ChatResponse {
            content: "Mock".to_string(),
            usage: None,
        })
    }
}

struct MockPlanner;
impl Planner for MockPlanner {
    fn plan(&self, _objective: &str) -> Result<Option<Vec<String>>, PlannerError> {
        Ok(None)
    }
}

#[derive(Clone)]
struct TestTaskRepository {
    host: Arc<BrioHostState>,
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
                        let assigned_agent = assigned_agent_str.map(AgentId::new);

                            Ok(Task::new(
                                TaskId::new(id as u64),
                                content,
                                Priority::new(priority as u8),
                                status,
                                None, // parent_id
                                assigned_agent,
                                HashSet::new(),
                            ))
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
                        let assigned_agent = assigned_agent_str.map(AgentId::new);

                        let parent_id_val: Option<i64> = r.try_get("parent_id").unwrap_or(None);
                        let parent_id = parent_id_val.map(|v| TaskId::new(v as u64));

                        Ok(Task::new(
                            TaskId::new(id as u64),
                            content,
                            Priority::new(priority as u8),
                            status,
                            parent_id,
                            assigned_agent,
                            HashSet::new(),
                        ))
                    })
                    .collect()
            })
        })
    }
}

struct TestDispatcher {
    host: Arc<BrioHostState>,
}

impl AgentDispatcher for TestDispatcher {
    fn dispatch(&self, agent: &AgentId, task: &Task) -> Result<DispatchResult, MeshError> {
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                self.host
                    .mesh_call(
                        agent.as_str(),
                        "fix",
                        Payload::Json(Box::new(task.content().to_string())),
                    )
                    .await
            })
        })
        .map(|_| DispatchResult::Accepted)
        .map_err(|e| MeshError::TransportError(e.to_string()))
    }
}

// =============================================================================
// The Test
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn manifesto_scenario_agent_fixing_bug() -> Result<()> {
    let mut env = TestEnvironment::setup().await?;
    let project_file = env.root.join("dummy_bug.txt");
    std::fs::write(&project_file, "bug")?;

    env.register_agent("agent_coder");
    let agent_rx = env.agent_msg_rx.take().expect("Agent receiver missing");
    let agent_host = env.host.clone();
    let project_path = env.root.to_string_lossy().to_string();

    tokio::spawn(async move {
        agent_logic(agent_host, agent_rx, project_path).await;
    });

    create_bug_fix_task(&env.host).await?;

    let processed = run_supervisor_cycle(env.host.clone()).await?;
    assert!(
        processed >= 1,
        "Should process at least 1 task state transition"
    );

    let content = std::fs::read_to_string(project_file)?;
    assert_eq!(content, "fixed");

    Ok(())
}

// Small Helper Functions (Small Functions, CQS)

async fn create_bug_fix_task(host: &BrioHostState) -> Result<()> {
    sqlx::query("INSERT INTO tasks (content, priority, status) VALUES (?, ?, ?)")
        .bind("fix bug")
        .bind(10)
        .bind("pending")
        .execute(host.db())
        .await?;
    Ok(())
}

async fn run_supervisor_cycle(host: Arc<BrioHostState>) -> Result<u32> {
    tokio::task::spawn_blocking(move || {
        let repo = TestTaskRepository { host: host.clone() };
        let dispatcher = TestDispatcher { host };
        let planner = MockPlanner;
        // Use default selector for test
        let selector = supervisor::selector::KeywordAgentSelector;
        let supervisor = Supervisor::new(repo, dispatcher, planner, selector);
        let mut total = 0;
        loop {
            let n = supervisor.poll_tasks().expect("Supervisor poll failed");
            if n == 0 {
                break;
            }
            total += n;
        }
        Ok(total)
    })
    .await?
}

// =============================================================================
// Smart Agent (Real AI)
// =============================================================================

async fn smart_agent_logic(
    host: Arc<BrioHostState>,
    mut rx: mpsc::Receiver<MeshMessage>,
    path: String,
) {
    use brio_kernel::inference::{Message, Role};

    while let Some(msg) = rx.recv().await {
        if msg.method == "fix" {
            // 1. Begin Session (Sandboxed)
            let session_id = host
                .begin_session(&path)
                .expect("Failed to begin session");
            let session_path = std::env::temp_dir().join("brio").join(&session_id);
            let file_path = session_path.join("dummy_bug.txt");

            // 2. Read Bug
            let content = std::fs::read_to_string(&file_path).expect("Failed to read bug file");

            // 3. Consult LLM (The "Smart" part)
            let prompt = format!(
                "You are an automated bug fixer. The file content is '{content}'. \
                Your task is to fix it by replacing the content with the word 'fixed'. \
                Return ONLY the result word 'fixed' with no other text, markdown, or explanation."
            );

            let request = ChatRequest {
                model: "mistralai/devstral-2512:free".to_string(), // Reliable free model
                messages: vec![
                    Message {
                        role: Role::System,
                        content: "You are a precise code editor.".into(),
                    },
                    Message {
                        role: Role::User,
                        content: prompt,
                    },
                ],
            };

            let response = host
                .inference()
                .expect("Default provider not found")
                .chat(request)
                .await
                .expect("LLM Call Failed");
            let fixed_content = response.content.trim(); // Trim potential whitespace

            // 4. Apply Fix
            std::fs::write(&file_path, fixed_content).expect("Failed to write fix");

            // 5. Commit
            host.commit_session(&session_id)
                .expect("Failed to commit session");
            let _ = msg
                .reply_tx
                .send(Ok(Payload::Json(Box::new(fixed_content.to_string()))));
        }
    }
}

async fn agent_logic(host: Arc<BrioHostState>, mut rx: mpsc::Receiver<MeshMessage>, path: String) {
    while let Some(msg) = rx.recv().await {
        if msg.method == "fix" {
            let session = host
                .begin_session(&path)
                .expect("Failed to begin session");
            let session_path = std::env::temp_dir().join("brio").join(&session);
            std::fs::write(session_path.join("dummy_bug.txt"), "fixed")
                .expect("Failed to write fix");
            host.commit_session(&session)
                .expect("Failed to commit session");
            let _ = msg.reply_tx.send(Ok(Payload::Json(Box::new("fixed".to_string()))));
        }
    }
}

// =============================================================================
// The "Real AI" Test
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn manifesto_scenario_real_ai() -> Result<()> {
    // 1. Check for Key (Conditional Execution)
    let api_key = if let Ok(k) = std::env::var("OPENROUTER_API_KEY") { k } else {
        println!("Skipping real AI test: OPENROUTER_API_KEY not set");
        return Ok(());
    };

    // 2. Setup Real Provider
    // Clean Code: Secure config
    let config = brio_kernel::inference::OpenAIConfig::new(
        secrecy::SecretString::new(api_key.into()),
        reqwest::Url::parse("https://openrouter.ai/api/v1/")?,
    );
    let provider = brio_kernel::inference::OpenAIProvider::new(config);

    let mut env = TestEnvironment::setup_with_provider(Box::new(provider)).await?;
    let project_file = env.root.join("dummy_bug.txt");
    std::fs::write(&project_file, "bug")?;

    env.register_agent("agent_coder");
    let agent_rx = env.agent_msg_rx.take().expect("Agent receiver missing");
    let agent_host = env.host.clone();
    let project_path = env.root.to_string_lossy().to_string();

    tokio::spawn(async move {
        smart_agent_logic(agent_host, agent_rx, project_path).await;
    });

    create_bug_fix_task(&env.host).await?;
    let processed = run_supervisor_cycle(env.host.clone()).await?;
    assert!(
        processed >= 1,
        "Should process at least 1 task state transition"
    );

    let content = std::fs::read_to_string(project_file)?;

    assert!(content.contains("fixed"), "LLM failed to fix the bug. Content: '{content}'");

    Ok(())
}
