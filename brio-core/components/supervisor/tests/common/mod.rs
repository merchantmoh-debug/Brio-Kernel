//! Test utilities for supervisor integration tests
//!
//! This module provides shared test infrastructure for the supervisor component,
//! including in-memory repositories and execution contexts.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use supervisor::branch::{BranchManager, BranchSource};
use supervisor::domain::{
    BranchConfig, BranchId, BranchRecord, BranchResult, BranchStatus, ExecutionMetrics,
    ExecutionStrategy, MergeRequest, Task,
};
use supervisor::merge::{MergeId, MergeStrategyRegistry};
use supervisor::mesh_client::{AgentDispatcher, DispatchResult, MeshError};
use supervisor::repository::{BranchRepository, BranchRepositoryError};

/// Mock dispatcher for testing
pub struct MockDispatcher;

impl AgentDispatcher for MockDispatcher {
    fn dispatch(
        &self,
        _agent: &supervisor::domain::AgentId,
        _task: &Task,
    ) -> Result<DispatchResult, MeshError> {
        Ok(DispatchResult::Accepted)
    }
}

/// Mock session manager for testing
pub struct MockSessionManager {
    sessions: HashMap<String, PathBuf>,
    next_session_id: u64,
}

impl MockSessionManager {
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
            next_session_id: 1,
        }
    }
}

impl supervisor::branch::SessionManager for MockSessionManager {
    fn begin_session(
        &mut self,
        base_path: &str,
    ) -> Result<String, supervisor::branch::SessionError> {
        let session_id = format!("session-{}", self.next_session_id);
        self.next_session_id += 1;
        self.sessions
            .insert(session_id.clone(), PathBuf::from(base_path));
        Ok(session_id)
    }

    fn commit_session(
        &mut self,
        _session_id: &str,
    ) -> Result<(), supervisor::branch::SessionError> {
        Ok(())
    }

    fn rollback_session(
        &mut self,
        session_id: &str,
    ) -> Result<(), supervisor::branch::SessionError> {
        self.sessions.remove(session_id);
        Ok(())
    }

    fn session_path(&self, session_id: &str) -> Option<PathBuf> {
        self.sessions.get(session_id).cloned()
    }

    fn active_session_count(&self) -> usize {
        self.sessions.len()
    }
}

/// Mock repository for testing branch operations
pub struct MockBranchRepository {
    branches: Mutex<HashMap<BranchId, BranchRecord>>,
    merge_requests: Mutex<HashMap<MergeId, MergeRequestRecord>>,
    next_merge_id: Mutex<u64>,
}

struct MergeRequestRecord {
    branch_id: BranchId,
    parent_id: Option<BranchId>,
    strategy: String,
    approved: bool,
    approved_by: Option<String>,
}

impl MockBranchRepository {
    pub fn new() -> Self {
        Self {
            branches: Mutex::new(HashMap::new()),
            merge_requests: Mutex::new(HashMap::new()),
            next_merge_id: Mutex::new(1),
        }
    }

    fn current_timestamp() -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64
    }
}

impl BranchRepository for MockBranchRepository {
    fn create_branch(&self, branch: &BranchRecord) -> Result<BranchId, BranchRepositoryError> {
        let mut branches = self
            .branches
            .lock()
            .map_err(|_| BranchRepositoryError::SqlError("Lock failed".to_string()))?;
        branches.insert(branch.id(), branch.clone());
        Ok(branch.id())
    }

    fn get_branch(&self, id: BranchId) -> Result<Option<BranchRecord>, BranchRepositoryError> {
        let branches = self
            .branches
            .lock()
            .map_err(|_| BranchRepositoryError::SqlError("Lock failed".to_string()))?;
        Ok(branches.get(&id).cloned())
    }

    fn update_branch_status(
        &self,
        id: BranchId,
        status: BranchStatus,
    ) -> Result<(), BranchRepositoryError> {
        let mut branches = self
            .branches
            .lock()
            .map_err(|_| BranchRepositoryError::SqlError("Lock failed".to_string()))?;

        if let Some(existing) = branches.get(&id) {
            let completed_at = if status.is_terminal() {
                Some(Self::current_timestamp())
            } else {
                existing.completed_at()
            };

            let config_json = existing.config().to_string();
            let updated = BranchRecord::new(
                existing.id(),
                existing.parent_id(),
                existing.session_id().to_string(),
                existing.name().to_string(),
                status,
                existing.created_at(),
                completed_at,
                config_json,
            )
            .map_err(|e| BranchRepositoryError::ParseError(e.to_string()))?;

            branches.insert(id, updated);
            Ok(())
        } else {
            Err(BranchRepositoryError::BranchNotFound(id))
        }
    }

    fn list_active_branches(&self) -> Result<Vec<BranchRecord>, BranchRepositoryError> {
        let branches = self
            .branches
            .lock()
            .map_err(|_| BranchRepositoryError::SqlError("Lock failed".to_string()))?;
        Ok(branches
            .values()
            .filter(|b: &&BranchRecord| b.is_active())
            .cloned()
            .collect())
    }

    fn list_branches_by_parent(
        &self,
        parent_id: BranchId,
    ) -> Result<Vec<BranchRecord>, BranchRepositoryError> {
        let branches = self
            .branches
            .lock()
            .map_err(|_| BranchRepositoryError::SqlError("Lock failed".to_string()))?;
        Ok(branches
            .values()
            .filter(|b: &&BranchRecord| b.parent_id() == Some(parent_id))
            .cloned()
            .collect())
    }

    fn delete_branch(&self, id: BranchId) -> Result<(), BranchRepositoryError> {
        let mut branches = self
            .branches
            .lock()
            .map_err(|_| BranchRepositoryError::SqlError("Lock failed".to_string()))?;
        branches.remove(&id);
        Ok(())
    }

    fn create_merge_request(
        &self,
        branch_id: BranchId,
        parent_id: Option<BranchId>,
        strategy: &str,
    ) -> Result<MergeId, BranchRepositoryError> {
        let mut merge_requests = self
            .merge_requests
            .lock()
            .map_err(|_| BranchRepositoryError::SqlError("Lock failed".to_string()))?;

        let mut next_id = self
            .next_merge_id
            .lock()
            .map_err(|_| BranchRepositoryError::SqlError("Lock failed".to_string()))?;

        let merge_id = MergeId::from_uuid(uuid::Uuid::from_u128(*next_id as u128));
        *next_id += 1;

        merge_requests.insert(
            merge_id,
            MergeRequestRecord {
                branch_id,
                parent_id,
                strategy: strategy.to_string(),
                approved: false,
                approved_by: None,
            },
        );

        Ok(merge_id)
    }

    fn get_merge_request(
        &self,
        merge_id: MergeId,
    ) -> Result<Option<MergeRequest>, BranchRepositoryError> {
        Err(BranchRepositoryError::SqlError(
            "Not implemented".to_string(),
        ))
    }

    fn update_merge_request(
        &self,
        _merge_request: &MergeRequest,
    ) -> Result<(), BranchRepositoryError> {
        Ok(())
    }

    fn approve_merge(
        &self,
        merge_id: MergeId,
        approver: &str,
    ) -> Result<(), BranchRepositoryError> {
        let mut merge_requests = self
            .merge_requests
            .lock()
            .map_err(|_| BranchRepositoryError::SqlError("Lock failed".to_string()))?;

        if let Some(record) = merge_requests.get_mut(&merge_id) {
            record.approved = true;
            record.approved_by = Some(approver.to_string());
            Ok(())
        } else {
            Err(BranchRepositoryError::BranchNotFound(BranchId::new()))
        }
    }
}

/// Default test files directory path
pub const TEST_FILES_DIR: &str = "./test_files";

/// Default merge strategy for tests
pub const DEFAULT_MERGE_STRATEGY: &str = "union";

/// Test context providing shared test infrastructure
pub struct TestContext {
    branch_manager: Arc<Mutex<BranchManager>>,
    repository: Arc<MockBranchRepository>,
}

impl TestContext {
    pub fn new() -> Self {
        let repository = Arc::new(MockBranchRepository::new());
        let session_manager = Arc::new(Mutex::new(MockSessionManager::new()));
        let merge_registry = MergeStrategyRegistry::new();

        let branch_manager = Arc::new(Mutex::new(BranchManager::new(
            session_manager.clone(),
            repository.clone(),
            merge_registry,
        )));

        Self {
            branch_manager,
            repository,
        }
    }

    pub fn branch_manager(&self) -> Arc<Mutex<BranchManager>> {
        self.branch_manager.clone()
    }

    pub async fn create_test_branch(&self, name: impl Into<String>) -> BranchId {
        let mut manager = self.branch_manager.lock().unwrap();
        manager
            .create_branch(
                BranchSource::Base(PathBuf::from(TEST_FILES_DIR)),
                Self::default_test_config(name),
            )
            .await
            .unwrap()
    }

    pub async fn create_and_complete_test_branch(&self) -> BranchId {
        let id = self.create_test_branch("Completed Branch").await;
        let manager = self.branch_manager.lock().unwrap();
        manager.mark_executing(id, 1).unwrap();

        let result = Self::default_test_result(id);
        manager.complete_branch(id, result).unwrap();
        id
    }

    pub async fn create_test_branch_with_file(
        &self,
        file_path: impl Into<String>,
        content: impl Into<String>,
    ) -> BranchId {
        let id = self.create_test_branch("Branch with File").await;
        // The file content would be written to the session path in a real implementation
        // For testing purposes, we just return the branch ID
        let _ = (file_path, content);
        id
    }

    /// Returns a default BranchConfig for testing
    pub fn default_test_config(name: impl Into<String>) -> BranchConfig {
        BranchConfig::new(
            name,
            vec![],
            ExecutionStrategy::Sequential,
            false,
            DEFAULT_MERGE_STRATEGY,
        )
        .unwrap()
    }

    /// Returns default ExecutionMetrics for testing
    pub fn default_test_metrics() -> ExecutionMetrics {
        ExecutionMetrics {
            total_duration_ms: 100,
            files_processed: 0,
            agents_executed: 1,
            peak_memory_bytes: 0,
        }
    }

    /// Returns a default BranchResult for testing with the given branch_id
    pub fn default_test_result(branch_id: BranchId) -> BranchResult {
        BranchResult::new(branch_id, vec![], vec![], Self::default_test_metrics())
    }
}

impl Default for TestContext {
    fn default() -> Self {
        Self::new()
    }
}
