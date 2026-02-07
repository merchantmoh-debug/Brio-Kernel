-- Migration: Add branch persistence tables for branching orchestrator feature
-- Creates tables for branches, executions, results, and merge queue
-- Uses TEXT for UUID storage (SQLite doesn't have a native UUID type)

-- Main branches table
CREATE TABLE IF NOT EXISTS branches (
    id TEXT PRIMARY KEY,  -- UUID stored as TEXT
    parent_id TEXT REFERENCES branches(id) ON DELETE CASCADE,
    session_id TEXT NOT NULL,
    name TEXT NOT NULL,
    status_json TEXT NOT NULL,  -- JSON-serialized BranchStatus
    config_json TEXT NOT NULL,  -- JSON-serialized BranchConfig
    created_at TEXT NOT NULL,  -- ISO8601 timestamp
    completed_at TEXT,  -- ISO8601 timestamp, NULL if not completed
    -- Composite index for efficient recovery after restart
    UNIQUE(session_id, name)
);

-- Index for parent-child lookups
CREATE INDEX IF NOT EXISTS idx_branches_parent ON branches(parent_id);

-- Index for session isolation
CREATE INDEX IF NOT EXISTS idx_branches_session ON branches(session_id);

-- Branch executions: tracks agent-task assignments within a branch
CREATE TABLE IF NOT EXISTS branch_executions (
    id TEXT PRIMARY KEY,  -- UUID stored as TEXT
    branch_id TEXT NOT NULL REFERENCES branches(id) ON DELETE CASCADE,
    agent_id TEXT NOT NULL,
    task_id TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'running', 'completed', 'failed', 'cancelled')),
    started_at TEXT,  -- ISO8601 timestamp
    completed_at TEXT,  -- ISO8601 timestamp
    -- Composite index for branch execution lookups
    UNIQUE(branch_id, agent_id, task_id)
);

-- Index for branch execution queries
CREATE INDEX IF NOT EXISTS idx_exec_branch ON branch_executions(branch_id);

-- Index for agent execution tracking
CREATE INDEX IF NOT EXISTS idx_exec_agent ON branch_executions(agent_id);

-- Branch results: stores aggregated results after branch completion
CREATE TABLE IF NOT EXISTS branch_results (
    branch_id TEXT PRIMARY KEY REFERENCES branches(id) ON DELETE CASCADE,
    file_changes_json TEXT NOT NULL DEFAULT '{}',
    agent_results_json TEXT NOT NULL DEFAULT '{}',
    created_at TEXT NOT NULL  -- ISO8601 timestamp
);

-- Merge queue: tracks merge requests and approvals
CREATE TABLE IF NOT EXISTS merge_queue (
    id TEXT PRIMARY KEY,  -- UUID stored as TEXT
    branch_id TEXT NOT NULL REFERENCES branches(id) ON DELETE CASCADE,
    strategy TEXT NOT NULL CHECK (strategy IN ('fast_forward', 'squash', 'merge_commit')),
    status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'approved', 'rejected', 'merged', 'conflict')),
    requires_approval INTEGER NOT NULL DEFAULT 1 CHECK (requires_approval IN (0, 1)),
    approved_by TEXT,
    approved_at TEXT,  -- ISO8601 timestamp
    created_at TEXT NOT NULL,  -- ISO8601 timestamp
    -- Each branch can have only one active merge request
    UNIQUE(branch_id)
);

-- Index for pending merge requests
CREATE INDEX IF NOT EXISTS idx_merge_status ON merge_queue(status) WHERE status IN ('pending', 'approved');

-- Index for approval queries
CREATE INDEX IF NOT EXISTS idx_merge_approver ON merge_queue(approved_by) WHERE approved_by IS NOT NULL;

-- Schema version tracking
CREATE TABLE IF NOT EXISTS schema_migrations (
    version INTEGER PRIMARY KEY,
    applied_at TEXT NOT NULL,  -- ISO8601 timestamp
    description TEXT
);

INSERT OR IGNORE INTO schema_migrations (version, applied_at, description) 
VALUES (2, datetime('now'), 'Add branch persistence tables for branching orchestrator');
