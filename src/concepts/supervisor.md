# Supervisor

The Supervisor is the orchestration brain of Brio-Kernel. It manages task lifecycle, coordinates agents, and implements sophisticated branching and merging workflows.

## What is the Supervisor?

The Supervisor is a **WebAssembly component** that serves as the central coordination layer for:

- **Task Management** - Fetching, dispatching, and tracking tasks
- **Agent Selection** - Intelligently matching tasks to appropriate agents
- **Branching Orchestration** - Enabling parallel task execution
- **Merge Coordination** - Handling conflict detection and resolution
- **State Management** - Maintaining task and branch state machines

```mermaid
graph TB
    subgraph "Supervisor Responsibilities"
        TM[Task Management]
        AS[Agent Selection]
        BO[Branching Orchestrator]
        MC[Merge Coordination]
        SM[State Management]
    end
    
    Task[Incoming Task] --> TM
    TM --> AS
    AS -->|Dispatch| Agent[Agent]
    
    TM -->|Complex Task| BO
    BO -->|Parallel Branches| Agents[Multiple Agents]
    Agents -->|Results| MC
    MC -->|Merge| Result[Final Result]
    
    TM -->|Updates| SM
    BO -->|Updates| SM
    MC -->|Updates| SM
```

## Architecture

The Supervisor uses **dependency injection** for all dependencies:

```mermaid
graph LR
    subgraph "Supervisor"
        Core[Core Logic]
    end
    
    subgraph "Dependencies via Traits"
        TaskRepo[TaskRepository]
        Dispatcher[AgentDispatcher]
        Planner[Planner]
        Selector[AgentSelector]
    end
    
    subgraph "Implementations"
        WitTask[WitTaskRepository]
        WitBranch[WitBranchRepository]
    end
    
    Core -->|Uses| TaskRepo
    Core -->|Uses| Dispatcher
    Core -->|Uses| Planner
    Core -->|Uses| Selector
    
    WitTask -->|Implements| TaskRepo
    WitBranch -->|Implements| TaskRepo
```

This design allows for:
- **Testing** - Mock implementations for unit tests
- **Flexibility** - Swappable implementations
- **WIT Integration** - Clean separation from host concerns

## Task Lifecycle

Tasks move through a well-defined state machine:

```mermaid
stateDiagram-v2
    [*] --> Pending: Task Created
    
    Pending --> Planning: Supervisor Active
    Planning --> Coordinating: Subtasks Created
    Coordinating --> Executing: Agent Assigned
    Executing --> Verifying: Agent Complete
    Verifying --> Completed: Verified
    
    Executing --> Failed: Error
    Verifying --> Failed: Verification Failed
    Failed --> Pending: Retry
    
    Completed --> [*]
```

### Task States

| State | Description | Transition To |
|-------|-------------|---------------|
| **Pending** | Waiting to be processed | Planning |
| **Planning** | Decomposing into subtasks | Coordinating |
| **Coordinating** | Assigning to agents | Executing |
| **Executing** | Agent is working | Verifying, Failed |
| **Verifying** | Validating results | Completed, Failed |
| **Completed** | Successfully finished | - |
| **Failed** | Error occurred | Pending (retry) |

### Branching Task States

For complex tasks requiring parallel execution:

```mermaid
stateDiagram-v2
    [*] --> AnalyzingForBranch: Complex Task
    AnalyzingForBranch --> Branching: Create Branches
    
    Branching --> Merging: All Complete
    Branching --> Branching: Progress Update
    
    Merging --> MergePendingApproval: Conflicts Detected
    Merging --> Completed: Clean Merge
    
    MergePendingApproval --> Completed: Approved
    MergePendingApproval --> Branching: Rejected
```

## Branching Orchestrator

The branching orchestrator enables **parallel execution** of tasks:

```mermaid
sequenceDiagram
    participant User
    participant Supervisor as Supervisor
    participant Branch1 as Branch A
    participant Branch2 as Branch B
    participant Branch3 as Branch C
    participant Merger as Merge Engine
    
    User->>Supervisor: Create complex task
    Supervisor->>Supervisor: Analyze for branching
    
    par Create Branches
        Supervisor->>Branch1: Create branch A
        Supervisor->>Branch2: Create branch B
        Supervisor->>Branch3: Create branch C
    end
    
    par Execute in Parallel
        Branch1->>Branch1: Execute strategy A
        Branch2->>Branch2: Execute strategy B
        Branch3->>Branch3: Execute strategy C
    end
    
    Branch1-->>Supervisor: Result A
    Branch2-->>Supervisor: Result B
    Branch3-->>Supervisor: Result C
    
    Supervisor->>Merger: Merge results
    Merger->>Merger: Detect conflicts
    
    alt No Conflicts
        Merger-->>Supervisor: Merged result
    else Conflicts Detected
        Merger-->>Supervisor: Conflicts for review
        Supervisor->>User: Request approval
        User-->>Supervisor: Approve/Modify
    end
    
    Supervisor-->>User: Final result
```

### Branch Configuration

```rust
pub struct BranchConfig {
    /// Human-readable branch name
    pub name: String,
    /// Assigned agents for this branch
    pub agents: Vec<AgentAssignment>,
    /// Execution strategy
    pub execution_strategy: ExecutionStrategy,
    /// Auto-merge without approval
    pub auto_merge: bool,
    /// Merge strategy to use
    pub merge_strategy: String,
}

pub enum ExecutionStrategy {
    /// Execute agents sequentially
    Sequential,
    /// Execute agents in parallel
    Parallel { max_concurrent: u32 },
}
```

### Example: Implementing a Feature

```toml
# Branch configuration for feature implementation
[[branches]]
name = "approach-a"
agents = ["coder", "reviewer"]
execution_strategy = { type = "Sequential" }
merge_strategy = "union"

[[branches]]
name = "approach-b"
agents = ["smart-agent"]
execution_strategy = { type = "Parallel", max_concurrent = 2 }
merge_strategy = "union"
auto_merge = false
```

## Merge Strategies

The Supervisor supports 4 merge strategies for combining branch results:

| Strategy | Description | Use Case |
|----------|-------------|----------|
| **Union** | Combines non-conflicting changes | Default, general purpose |
| **Ours** | Prefers base version on conflict | Conservative updates |
| **Theirs** | Prefers branch version on conflict | Accept branch changes |
| **Three-Way** | Line-level conflict detection | Precise control |

```mermaid
graph TB
    subgraph "Merge Strategies"
        Base[Base Version]
        BranchA[Branch A Changes]
        BranchB[Branch B Changes]
        
        Base --> Union[Union Strategy]
        BranchA --> Union
        BranchB --> Union
        
        Union -->|No Conflict| Result[Combined Result]
        Union -->|Conflict| Ours[Ours Strategy]
        Union -->|Conflict| Theirs[Theirs Strategy]
        Union -->|Conflict| ThreeWay[Three-Way Strategy]
        
        Ours --> Result
        Theirs --> Result
        ThreeWay -->|Resolved| Result
        ThreeWay -->|Unresolved| Conflict[Conflict Markers]
    end
```

See [Merge Strategies](../guides/merge-strategies.md) for detailed documentation.

## Agent Selection

The Supervisor intelligently matches tasks to agents based on:

### Capability-Based Selection

```mermaid
graph LR
    Task[Task Requirements] -->|Needs| Capabilities[Capabilities]
    Capabilities -->|Match| Agents[Available Agents]
    Agents -->|Select Best| Selected[Selected Agent]
```

**Example:**
```rust
// Task requires file writing
if task.requires_write_access() {
    candidates.retain(|a| a.has_capability("write_file"));
}

// Task involves shell commands
if task.requires_shell() {
    candidates.retain(|a| a.has_capability("shell"));
}
```

### Selection Methods

1. **Capability-Based** (default) - Match task requirements to agent capabilities
2. **Round-Robin** - Distribute evenly across agents
3. **Random** - Random selection (for testing)

## Transaction System

The Supervisor implements database transactions for atomic operations:

```mermaid
sequenceDiagram
    participant Supervisor
    participant TX as Transaction
    participant Repo as Repository
    participant DB as SQLite
    
    Supervisor->>TX: begin()
    TX-->>Supervisor: Transaction handle
    
    Supervisor->>Repo: create_task(tx, data)
    Repo->>DB: INSERT
    
    Supervisor->>Repo: create_subtasks(tx, tasks)
    Repo->>DB: Multiple INSERTs
    
    Supervisor->>TX: commit()
    TX->>DB: COMMIT
    TX-->>Supervisor: Success
    
    Note over Supervisor,DB: All operations succeed or all fail
```

### Transaction Safety

- **Auto-Rollback**: Transactions automatically rollback if not committed
- **Error Handling**: Proper error conversion and propagation
- **WIT Compatibility**: Works with WIT sql-state interface

```rust
// Manual control
let mut tx = Transaction::begin()?;
tx.execute("INSERT INTO tasks ...", &params)?;
tx.commit()?;

// Closure API (auto commit/rollback)
repo.with_transaction(|tx| {
    tx.execute("INSERT ...", &params)?;
    tx.execute("UPDATE ...", &params)?;
    Ok(())
})?;
```

## Domain Model

### Core Entities

```mermaid
erDiagram
    TASK ||--o{ TASK : "parent/child"
    TASK ||--|| TASK_STATUS : has
    TASK ||--o{ CAPABILITY : requires
    TASK ||--o| AGENT : assigned
    
    BRANCH ||--o{ BRANCH : "parent/children"
    BRANCH ||--|| BRANCH_STATUS : has
    BRANCH ||--|| BRANCH_CONFIG : configured
    BRANCH ||--o{ AGENT_ASSIGNMENT : uses
    
    MERGE_REQUEST ||--|| MERGE_REQUEST_STATUS : has
    MERGE_REQUEST ||--o{ CONFLICT : contains
    MERGE_REQUEST ||--o{ FILE_CHANGE : tracks
    
    TASK ||--o{ BRANCH : creates
    BRANCH ||--o{ MERGE_REQUEST : produces
```

### Key Types

**Task:**
```rust
pub struct Task {
    pub id: TaskId,
    pub content: String,
    pub priority: Priority,
    pub status: TaskStatus,
    pub parent_id: Option<TaskId>,
    pub assigned_agent: Option<AgentId>,
    pub required_capabilities: HashSet<Capability>,
}
```

**Branch:**
```rust
pub struct Branch {
    pub id: BranchId,
    pub parent_id: Option<BranchId>,
    pub session_id: String,
    pub name: String,
    pub status: BranchStatus,
    pub children: Vec<BranchId>,
    pub config: BranchConfig,
    pub created_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub execution_result: Option<BranchResult>,
    pub merge_result: Option<MergeResult>,
}
```

## Configuration

### TOML Configuration

```toml
[supervisor]
enabled = true
poll_interval = 5  # seconds

# Branching
max_concurrent_branches = 5
auto_merge = false

# Merge strategy
merge_strategy = "union"

# Agent selection
[supervisor.agent_selection]
method = "capability"
```

### Environment Variables

```bash
export BRIO_SUPERVISOR_ENABLED=true
export BRIO_SUPERVISOR_POLL_INTERVAL=5
export BRIO_SUPERVISOR_MAX_CONCURRENT_BRANCHES=5
export BRIO_SUPERVISOR_AUTO_MERGE=false
export BRIO_SUPERVISOR_MERGE_STRATEGY="union"
```

## WIT Interfaces

The Supervisor implements these WIT interfaces:

### Required Imports

- `sql-state` - Database operations
- `service-mesh` - Inter-component communication
- `logging` - Structured logging

### Implementation

```wit
world supervisor {
  import sql-state;
  import service-mesh;
  import logging;
  
  // Supervisor logic implemented in Rust
  // Compiled to WASM component
}
```

## Best Practices

### Task Design

1. **Clear Descriptions** - Tasks should be specific and actionable
2. **Appropriate Scope** - Break large tasks into smaller ones
3. **Capability Requirements** - Specify required capabilities
4. **Priority Levels** - Use priority for task ordering

### Branching

1. **Start Simple** - Use branching for complex tasks only
2. **Limit Concurrent Branches** - Prevent resource exhaustion
3. **Auto-merge Cautiously** - Review before enabling
4. **Choose Strategy Wisely** - Match strategy to use case

### Error Handling

1. **Retry Failed Tasks** - Implement retry logic
2. **Monitor Timeouts** - Set appropriate limits
3. **Log Everything** - Use structured logging
4. **Alert on Failures** - Integrate with monitoring

## Monitoring

### Metrics

- Tasks created/completed/failed per minute
- Average task execution time
- Branch creation and merge rates
- Conflict detection rates
- Agent utilization

### Health Checks

```bash
# Check supervisor health
curl http://localhost:8080/health/supervisor

# Get task statistics
curl http://localhost:8080/api/supervisor/stats
```

## Troubleshooting

### Common Issues

**Tasks Not Being Processed**
- Check supervisor is enabled: `BRIO_SUPERVISOR_ENABLED=true`
- Verify database connectivity
- Check agent availability

**Branches Stuck in Merging**
- Check for unresolved conflicts
- Verify merge strategy configuration
- Review branch logs for errors

**Agent Selection Failing**
- Verify agents are registered
- Check capability requirements
- Review agent health status

## Additional Resources

- **[Branching Workflows](../guides/branching-workflows.md)** - Detailed branching guide
- **[Merge Strategies](../guides/merge-strategies.md)** - Complete merge strategy documentation
- **[Creating Agents](../guides/creating-agents.md)** - Build agents that work with Supervisor
- **[Agent SDK](../api-reference/agent-sdk.md)** - SDK for agent development

---

The Supervisor is the orchestration layer that makes Brio's multi-agent system possible. Its combination of task management, branching, and merge capabilities enables complex workflows that would be difficult to coordinate manually.
