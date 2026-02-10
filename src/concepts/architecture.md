# Architecture

Brio-Kernel is designed as a strictly headless micro-kernel that orchestrates AI agents using the WebAssembly Component Model. This document explains the architecture, design principles, and system components.

## Design Principles

### 1. Security-First

All components run in sandboxed WebAssembly with capability-based access control:

- **WASM Sandbox**: Components cannot access host resources directly
- **Capability Model**: Each component declares required capabilities
- **Policy Enforcement**: SQL and VFS access is controlled at runtime
- **Input Validation**: All inputs are validated before processing

### 2. Zero-Copy Performance

Internal communication uses direct memory channels instead of serialization:

- **No HTTP Between Components**: Uses tokio mpsc channels
- **Shared Memory**: Large data transfers use zero-copy techniques
- **Minimal Serialization**: Protocol Buffers only at network boundaries

### 3. Atomic Operations

File changes are atomic with rollback capability:

- **Session-Based VFS**: Changes isolated in temp directories
- **Atomic Commit**: Move operations ensure consistency
- **Automatic Rollback**: Failed operations clean up automatically

## System Architecture

```mermaid
graph TB
    subgraph "Client Layer"
        TUI[TUI Client]
        Web[Web UI]
        IDE[IDE Plugin]
    end
    
    subgraph "Brio Kernel"
        WS[WebSocket<br/>Broadcaster]
        Host[BrioHostState<br/>Central Orchestrator]
        Mesh[Service Mesh<br/>IPC Layer]
        VFS[VFS Session<br/>Manager]
        SQL[SQL Store<br/>SQLite]
        INF[Inference<br/>Provider]
    end
    
    subgraph "WASM Components"
        Supervisor[Supervisor<br/>Policy Engine]
        Agents[Agents<br/>Coder/Reviewer/etc.]
        Tools[Tools<br/>Shell/Grep/Read]
    end
    
    subgraph "External Services"
        LLM[LLM APIs<br/>OpenAI/Anthropic]
        Git[Git Repository]
        DB[(SQLite<br/>State Store)]
    end
    
    TUI -->|WebSocket| WS
    Web -->|WebSocket| WS
    IDE -->|WebSocket| WS
    
    WS --> Host
    Host --> Mesh
    Host --> VFS
    Host --> SQL
    Host --> INF
    
    Mesh --> Supervisor
    Mesh --> Agents
    Mesh --> Tools
    
    Supervisor --> SQL
    Agents --> VFS
    Agents --> Tools
    Tools --> VFS
    
    INF -->|HTTP| LLM
    VFS --> Git
    SQL --> DB
```

## Core Subsystems

### 1. BrioHostState

The central orchestrator managing all system state:

```mermaid
graph LR
    subgraph "BrioHostState"
        Router[Mesh Router]
        DB[(DB Pool)]
        BC[Broadcaster]
        SM[Session Manager]
        PR[Provider Registry]
        EB[Event Bus]
        BM[Branch Manager]
    end
    
    Router -->|Routes to| Components
    Components -->|Queries| DB
    Components -->|Events| EB
    EB -->|Broadcasts| BC
    SM -->|Manages| Sessions
    PR -->|Provides| LLM
    BM -->|Manages| Branches
```

**Responsibilities:**
- Component lifecycle management
- Task routing and dispatch
- State synchronization
- Resource allocation

### 2. Service Mesh (IPC)

Zero-copy inter-component communication:

```mermaid
sequenceDiagram
    participant Client as Agent/Tool
    participant Mesh as Service Mesh
    participant Target as Target Component
    
    Client->>Mesh: mesh_call(target, method, payload)
    Mesh->>Mesh: Route to target
    Mesh->>Target: Invoke method
    Target-->>Mesh: Return result
    Mesh-->>Client: Return result
```

**Features:**
- Local routing via tokio mpsc
- Remote routing via gRPC (distributed mode)
- Automatic service discovery
- Load balancing
- Circuit breakers

### 3. VFS Session Manager

Copy-on-write workspace isolation:

```mermaid
stateDiagram-v2
    [*] --> Pending: begin_session()
    Pending --> Active: Session created
    Active --> Committing: commit_session()
    Active --> RollingBack: rollback_session()
    Committing --> Committed: Atomic move
    RollingBack --> RolledBack: Cleanup
    Committed --> [*]
    RolledBack --> [*]
```

**Workflow:**
1. `begin_session(base_path)` - Creates sandbox copy
2. Agent writes to `/tmp/brio/sess-{id}/`
3. `commit_session(id)` - Atomic move to original
4. `rollback_session(id)` - Discard changes

**Features:**
- Directory hashing for conflict detection
- Reflink support (copy-on-write) when available
- Automatic cleanup on failure

### 4. SQL Store

Policy-enforced SQLite access:

```mermaid
graph LR
    Agent[Agent] -->|Query| Policy[Policy Checker]
    Policy -->|Validate| SQL[SQLite]
    SQL -->|Results| Agent
```

**Policy System:**
- Prefix-based table scoping
- Query validation with sqlparser
- Agent "coder" can only access `coder_*` tables
- Row-level security support

### 5. WebSocket Broadcaster

Real-time state distribution:

```mermaid
sequenceDiagram
    participant Server as Brio Server
    participant Broadcaster as WebSocket Broadcaster
    participant Client1 as Client A
    participant Client2 as Client B
    
    Server->>Broadcaster: State change
    Broadcaster->>Broadcaster: Generate JSON Patch
    Broadcaster->>Client1: Patch update
    Broadcaster->>Client2: Patch update
```

**Format:** JSON Patch (RFC 6902)
```json
{
  "op": "replace",
  "path": "/tasks/0/status",
  "value": "completed"
}
```

### 6. Inference Provider System

Multi-model LLM abstraction:

```mermaid
graph TB
    Agent[Agent] -->|chat()| Registry[Provider Registry]
    Registry -->|Select| OpenAI[OpenAI Provider]
    Registry -->|Select| Anthropic[Anthropic Provider]
    Registry -->|Select| Local[Local Provider]
    
    OpenAI -->|HTTP| OpenAI_API[OpenAI API]
    Anthropic -->|HTTP| Anthropic_API[Anthropic API]
    Local -->|HTTP| Ollama[Ollama/etc.]
```

**Features:**
- Retry logic with exponential backoff
- Rate limiting
- Circuit breaker pattern
- Provider fallback

### 7. WASM Component Runtime

Powered by Wasmtime:

```mermaid
graph LR
    WASM[WASM Component] -->|WIT Interface| Runtime[Wasmtime Runtime]
    Runtime -->|Implements| Host[Host Functions]
    
    subgraph "Host Functions"
        SQL_H[sql-state]
        FS_H[session-fs]
        Mesh_H[service-mesh]
        Log_H[logging]
    end
```

## Data Flow Example

### Scenario: User Requests Bug Fix

```mermaid
sequenceDiagram
    participant User
    participant WS as WebSocket
    participant Host as BrioHostState
    participant SQL as SQL Store
    participant Supervisor as Supervisor
    participant VFS as VFS Session
    participant Agent as Coder Agent
    participant Tool as Tools
    
    User->>WS: "Fix auth bug"
    WS->>Host: Create task
    Host->>SQL: INSERT INTO tasks
    Host->>WS: Broadcast: task created
    
    Host->>Supervisor: Activate
    Supervisor->>SQL: SELECT pending tasks
    Supervisor->>Host: Dispatch to coder agent
    
    Host->>VFS: begin_session("./src")
    VFS->>Host: session_id = "abc-123"
    
    Host->>Agent: Run with session
    
    loop Agent Execution
        Agent->>Tool: grep("auth", "./src")
        Tool->>VFS: Read files
        Tool-->>Agent: Matches
        
        Agent->>Tool: read_file("auth.rs")
        Tool->>VFS: Read file
        Tool-->>Agent: Content
        
        Agent->>Host: Request LLM inference
        Host->>Agent: Response
    end
    
    Agent->>VFS: Write fix to session
    Agent->>Host: Signal done
    
    Host->>VFS: commit_session("abc-123")
    VFS->>Host: Changes committed
    
    Host->>SQL: UPDATE task status
    Host->>WS: Broadcast: task completed
    WS->>User: Completion notification
```

## Directory Structure

```
brio-core/
├── Cargo.toml              # Workspace manifest
├── Cargo.lock
├── deny.toml               # Dependency policy
├── wit/                    # WIT interface definitions
│   ├── host.wit           # Core host interfaces
│   ├── mesh.wit           # Service mesh interfaces
│   ├── tools.wit          # Tool interfaces
│   └── brio.wit           # Main world definitions
├── kernel/                 # Rust host implementation
│   └── src/
│       ├── host/          # Host state and orchestration
│       ├── engine/        # Wasmtime runtime setup
│       ├── mesh/          # Service mesh implementation
│       ├── store/         # SQL store
│       ├── vfs/           # VFS session manager
│       ├── ws/            # WebSocket broadcaster
│       ├── inference/     # LLM provider abstraction
│       └── infrastructure/# Config, telemetry, server
├── components/             # WASM components
│   ├── supervisor/        # Policy engine
│   ├── agents/            # Agent implementations
│   │   ├── coder/
│   │   ├── reviewer/
│   │   ├── council/
│   │   ├── foreman/
│   │   └── smart-agent/
│   ├── agent-sdk/         # Shared SDK library
│   └── tools/             # Tool implementations
│       ├── shell-tool/
│       ├── tool_grep/
│       └── tool_read_file/
├── integration-tests/      # Integration tests
└── benches/               # Performance benchmarks
```

## Security Model

### Capability-Based Access Control

```mermaid
graph TB
    subgraph "Component Capabilities"
        Supervisor_C[sql-state<br/>service-mesh<br/>logging]
        Agent_C[session-fs<br/>service-mesh<br/>logging]
        Tool_C[session-fs<br/>read-only<br/>logging]
    end
    
    subgraph "Host Enforcement"
        Check[Capability<br/>Checker]
        SQL_P[SQL Policy]
        VFS_P[VFS Policy]
    end
    
    Supervisor_C --> Check
    Agent_C --> Check
    Tool_C --> Check
    Check --> SQL_P
    Check --> VFS_P
```

### Isolation Layers

1. **WASM Sandbox**: Memory isolation, no direct host access
2. **VFS Sessions**: File write isolation
3. **SQL Policy**: Table-level access control
4. **Permission Checker**: Runtime validation

## Performance Characteristics

| Metric | Target | Notes |
|--------|--------|-------|
| Task Dispatch Latency | <10ms | Local mesh routing |
| VFS Session Creation | <100ms | With reflink support |
| File Commit | <50ms | Atomic move |
| WebSocket Broadcast | <5ms | JSON patch diff |
| Agent Startup | <500ms | WASM instantiation |
| LLM Response | Varies | Depends on provider |

## Scalability Considerations

### Current Limits

- **Concurrent Agents**: Limited by system resources (default: 10)
- **File Size**: 10MB per file (configurable)
- **Session Count**: Limited by disk space
- **WebSocket Connections**: 1000+ (configurable)

### Distributed Mode

For horizontal scaling:

```mermaid
graph TB
    subgraph "Node 1"
        Host1[BrioHostState]
        Agent1[Agents]
    end
    
    subgraph "Node 2"
        Host2[BrioHostState]
        Agent2[Agents]
    end
    
    subgraph "Node 3"
        Host3[BrioHostState]
        Agent3[Agents]
    end
    
    Host1 -->|gRPC| Host2
    Host2 -->|gRPC| Host3
    Host1 -->|gRPC| Host3
```

## Future Enhancements

### Implemented ✅

- ~~Distributed Mesh~~ - Multi-node service mesh
- ~~Multi-Model Support~~ - Concurrent LLM providers

### Planned 🚧

- **Component Hot-Reload** - Update agents without restart
- **Persistent Sessions** - Resume across restarts
- **Plugin System** - Third-party tool installation
- **Advanced Scheduling** - Priority queues, resource limits

## Additional Resources

- **[Security Model](../concepts/security-model.md)** - Detailed security documentation
- **[WIT Interfaces](../concepts/wit-interfaces.md)** - Interface definitions
- **[Distributed Mesh](../guides/distributed-mesh.md)** - Multi-node deployment
- **[Creating Agents](../guides/creating-agents.md)** - Build custom agents

---

This architecture enables Brio to provide a secure, high-performance platform for AI agent orchestration with strong isolation guarantees and flexible extensibility.
