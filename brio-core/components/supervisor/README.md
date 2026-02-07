# Supervisor Component

The Supervisor is the orchestration brain of Brio. It manages task lifecycle, agent dispatch, and branching execution strategies.

## Overview

The Supervisor component provides:

- **Task Management**: Fetching, dispatching, and tracking task execution
- **Branching Orchestrator**: Parallel execution of tasks across multiple isolated branches
- **Agent Selection**: Intelligent matching of tasks to appropriate agents
- **Merge Strategies**: Conflict resolution for combining branch results

## Architecture

```
supervisor/
├── src/
│   ├── branch/          # Branching orchestrator
│   │   ├── manager.rs   # Branch lifecycle management
│   │   ├── execution.rs # Parallel execution engine
│   │   ├── mod.rs       # Branch domain types
│   │   └── events.rs    # Branch event system
│   ├── domain.rs        # Core domain types (Task, Branch, AgentId)
│   ├── handlers/        # Task state handlers
│   ├── merge.rs         # Merge strategies
│   ├── orchestrator.rs  # Main supervisor loop
│   ├── repository.rs    # Data access layer
│   └── selector.rs      # Agent selection logic
```

## Branching Orchestrator

The branching orchestrator enables parallel execution of tasks with automatic conflict detection and resolution.

### Configuration

Add to your `config.toml`:

```toml
[branching]
max_concurrent_branches = 8
default_merge_strategy = "union"
allow_nested_branches = true
auto_merge = false
require_merge_approval = true
branch_timeout_secs = 300
line_level_diffs = true
max_nesting_depth = 3
```

### Environment Variables

- `BRIO_MAX_BRANCHES` - Override max concurrent branches
- `BRIO_AUTO_MERGE` - Enable auto-merge (true/false)
- `BRIO_MERGE_TIMEOUT` - Timeout in seconds for merge operations
- `BRIO_MERGE_STRATEGY` - Default merge strategy (union, ours, theirs)

### Merge Strategies

Available merge strategies:

- **union**: Combine non-conflicting changes, mark conflicts
- **ours**: Prefer base version on conflict
- **theirs**: Prefer branch version on conflict

### Usage Example

```rust
use supervisor::branch::manager::{BranchManager, BranchSource};
use supervisor::domain::{BranchConfig, ExecutionStrategy};

// Create a branch
let branch_id = branch_manager
    .create_branch(
        BranchSource::Base("./workspace".into()),
        BranchConfig {
            name: "Feature Branch".to_string(),
            agents: vec![agent_assignment],
            execution_strategy: ExecutionStrategy::Parallel { max_concurrent: 4 },
            auto_merge: false,
            merge_strategy: "union".to_string(),
        },
    )
    .await?;

// Execute branch
branch_manager.mark_executing(branch_id, 1)?;
// ... agents execute ...

// Complete branch
branch_manager.complete_branch(branch_id, results)?;

// Request merge
let merge_id = branch_manager
    .request_merge(branch_id, "union", true)
    .await?;

// Approve and execute merge
branch_manager.approve_merge(merge_id, "user@example.com")?;
```

## Testing

Run the integration tests:

```bash
cd brio-core/components/supervisor
cargo test --test branch_integration_tests
```

Run all tests:

```bash
cargo test
```

### Test Coverage

The integration tests cover:

- **Branch Lifecycle**: Creation, execution, completion
- **Parallel Execution**: Multiple branches executing concurrently
- **Max Branch Limits**: Enforcing concurrent branch limits
- **Merge Workflows**: Approval requirements and execution
- **Nested Branches**: Hierarchical branch structures
- **Recovery**: State recovery after restart
- **Error Handling**: Invalid transitions and edge cases

## Building

The Supervisor is built as a WASI Preview 2 component:

```bash
# Build the WASM component
cargo component build --release
```

## Dependencies

- `wit-bindgen`: WIT bindings for WASM component interface
- `serde`: Serialization for domain types
- `uuid`: Unique identifiers
- `chrono`: Timestamp handling
- `tokio`: Async runtime (for tests)
- `thiserror`: Error handling

## License

MPL-2.0
