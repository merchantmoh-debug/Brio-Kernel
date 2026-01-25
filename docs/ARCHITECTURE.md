# Brio Kernel Architecture

This document provides an in-depth overview of the Brio Kernel architecture, its design principles, and subsystem interactions.

---

## Design Philosophy

Brio follows three core principles:

1. **Security-First**: All components run in sandboxed WebAssembly with capability-based access control
2. **Zero-Copy Performance**: Internal IPC uses direct memory channels, not HTTP/serialization
3. **Atomic Operations**: File changes are isolated in temp directories and atomically committed

---

## System Architecture

```
                                    ┌──────────────────────────────┐
                                    │      External Clients        │
                                    │   (TUI, Web UI, IDE Plugin)  │
                                    └──────────────┬───────────────┘
                                                   │ WebSocket (JSON Patches)
                                                   ▼
┌──────────────────────────────────────────────────────────────────────────────┐
│                              BRIO KERNEL                                      │
│                                                                               │
│  ┌─────────────────────────────────────────────────────────────────────────┐ │
│  │                         Control Plane (Axum Server)                      │ │
│  │  • WebSocket endpoint for real-time state sync                          │ │
│  │  • REST API for configuration and management                            │ │
│  └─────────────────────────────────────────────────────────────────────────┘ │
│                                      │                                        │
│  ┌───────────────────────────────────┴───────────────────────────────────┐   │
│  │                         BrioHostState (Orchestrator)                   │   │
│  │  ┌─────────────┐ ┌─────────────┐ ┌─────────────┐ ┌─────────────────┐  │   │
│  │  │ Broadcaster │ │SessionManager│ │  SqlStore   │ │  LLMProvider   │  │   │
│  │  │(WS Patches) │ │(VFS Sandbox)│ │ (State DB)  │ │  (Inference)   │  │   │
│  │  └─────────────┘ └─────────────┘ └─────────────┘ └─────────────────┘  │   │
│  └───────────────────────────────────────────────────────────────────────┘   │
│                                      │                                        │
│  ┌───────────────────────────────────┴───────────────────────────────────┐   │
│  │                          Service Mesh (IPC Router)                     │   │
│  │                    Direct Tokio mpsc channels, zero-copy               │   │
│  └─────────────┬───────────────────────────────────────┬─────────────────┘   │
│                │                                       │                      │
│  ┌─────────────▼─────────────┐           ┌────────────▼────────────────┐    │
│  │   WASM Component Runtime   │           │   WASM Component Runtime    │    │
│  │        (Wasmtime)          │           │        (Wasmtime)           │    │
│  │  ┌─────────────────────┐  │           │  ┌──────────────────────┐   │    │
│  │  │     Supervisor      │  │           │  │       Agents         │   │    │
│  │  │  (Policy Engine)    │  │           │  │  (Stateful Workers)  │   │    │
│  │  └─────────────────────┘  │           │  └──────────────────────┘   │    │
│  │  ┌─────────────────────┐  │           │  ┌──────────────────────┐   │    │
│  │  │       Tools         │  │           │  │       Tools          │   │    │
│  │  │ (Stateless Funcs)   │  │           │  │   (grep, read, etc.) │   │    │
│  │  └─────────────────────┘  │           │  └──────────────────────┘   │    │
│  └───────────────────────────┘           └─────────────────────────────┘    │
│                                                                               │
└──────────────────────────────────────────────────────────────────────────────┘
                    │                                      │
                    ▼                                      ▼
            ┌──────────────┐                      ┌──────────────────┐
            │   brio.db    │                      │  /tmp/brio/      │
            │   (SQLite)   │                      │  (VFS Sessions)  │
            └──────────────┘                      └──────────────────┘
```

---

## Core Subsystems

### 1. BrioHostState

The central orchestrator managing all kernel subsystems.

| Component            | Purpose                                 |
| -------------------- | --------------------------------------- |
| `mesh_router`        | Routes messages between WASM components |
| `db_pool`            | SQLite connection pool                  |
| `broadcaster`        | WebSocket state distribution            |
| `session_manager`    | VFS sandbox lifecycle                   |
| `inference_provider` | LLM API abstraction                     |

**Key Methods:**
- `mesh_call()` - Send message to component via service mesh
- `begin_session()` / `commit_session()` - VFS lifecycle
- `broadcast_patch()` - Push state update to all clients

---

### 2. Service Mesh

Internal IPC system replacing external message brokers.

```rust
pub struct MeshMessage {
    target: String,              // Component ID (e.g., "tool_grep")
    method: String,              // Method name (e.g., "search")
    payload: Payload,            // JSON or binary data
    reply_tx: oneshot::Sender<Result<Payload, String>>,
}

pub enum Payload {
    Json(String),
    Binary(Vec<u8>),
}
```

**Flow:**
1. Caller invokes `mesh_call("tool_grep", "search", payload)`
2. Host routes to registered component channel
3. Component processes request, sends reply via `oneshot`
4. Caller receives response

---

### 3. VFS Session Manager

Implements copy-on-write workspace isolation.

**Lifecycle:**

```
┌─────────────┐     begin_session()     ┌─────────────────────┐
│   ./src     │ ──────────────────────▶ │ /tmp/brio/sess-123  │
│  (Original) │     (cp -r or reflink)  │    (Sandbox Copy)   │
└─────────────┘                         └─────────────────────┘
                                                  │
                                           Agent writes files
                                                  │
                                                  ▼
                                        ┌─────────────────────┐
                                        │  Changes in sandbox │
                                        └─────────────────────┘
                                                  │
                           ┌──────────────────────┼──────────────────────┐
                           │                      │                      │
                    commit_session()       rollback_session()     (crash recovery)
                           │                      │                      │
                           ▼                      ▼                      ▼
                   ┌──────────────┐      ┌──────────────┐      ┌──────────────┐
                   │ Atomic move  │      │  Discard all │      │ cleanup_     │
                   │ to original  │      │   changes    │      │ orphaned()   │
                   └──────────────┘      └──────────────┘      └──────────────┘
```

**Conflict Detection:**
- Directory hash computed at session start
- Compared at commit time
- Commit fails if base changed (prevents lost updates)

---

### 4. SQL Store

Policy-enforced SQLite access for relational state.

```rust
pub trait QueryPolicy: Send + Sync {
    fn authorize(&self, scope: &str, sql: &str) -> Result<(), PolicyError>;
}
```

**Current Policy**: `PrefixPolicy` - Ensures agents can only access tables prefixed with their scope.

**Example:**
```sql
-- Supervisor (scope: "supervisor")
SELECT * FROM supervisor_tasks WHERE status = 'pending'

-- Agent (scope: "agent_coder") 
-- Can only access: agent_coder_* tables
```

---

### 5. WebSocket Broadcaster

Real-time state distribution using JSON Patches.

```rust
pub enum BroadcastMessage {
    Patch(WsPatch),
    Shutdown,
}

pub struct WsPatch {
    pub path: String,           // JSON Pointer (RFC 6901)
    pub op: PatchOperation,     // add, remove, replace
    pub value: serde_json::Value,
}
```

**Why JSON Patches?**
- Minimal bandwidth (only changes, not full state)
- Standard format (RFC 6902)
- Easy client-side application

---

### 6. Inference Provider

Abstraction layer for LLM API calls.

```rust
#[async_trait]
pub trait LLMProvider: Send + Sync {
    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse, InferenceError>;
}
```

**OpenAI Implementation Features:**
- Exponential backoff with jitter
- Rate limit handling (429 responses)
- Configurable retry count
- Context length validation

---

### 7. WASM Component Runtime

Wasmtime-based execution of WASI Preview 2 components.

**Component Types:**

| Type           | Characteristics          | Example                          |
| -------------- | ------------------------ | -------------------------------- |
| **Supervisor** | Stateful, policy engine  | Task scheduling, agent selection |
| **Agent**      | Stateful, long-running   | Code analysis, file editing      |
| **Tool**       | Stateless, pure function | Grep, file read, shell execute   |

**WIT Interfaces Imported by Components:**
- `service-mesh` - Call other components
- `sql-state` - Query/execute SQL
- `session-fs` - Begin/commit sessions
- `wasi:logging` - Structured logging

---

## Data Flow Examples

### Example 1: User Requests Bug Fix

```
1. User → WebSocket → "Fix auth bug"
                           │
2. Control Plane receives task
   Creates task record in SQLite
   Broadcasts: { "op": "add", "path": "/tasks/-", "value": {...} }
                           │
3. Supervisor component activated
   Queries: SELECT agent_id FROM agents WHERE specialty = 'code'
   Selects agent_coder
                           │
4. Host creates VFS session
   begin_session("./src") → /tmp/brio/sess-abc
                           │
5. Agent receives task via mesh_call
   Calls tool_grep via mesh
   Reads files, analyzes code
                           │
6. Agent writes fix to session sandbox
   /tmp/brio/sess-abc/auth.rs
                           │
7. Agent signals completion
   Host commits session (atomic move)
   Broadcasts: { "op": "replace", "path": "/tasks/0/status", "value": "completed" }
                           │
8. Client receives patch, updates UI
```

---

## Configuration

**Location:** `kernel/src/infrastructure/config.rs`

```rust
pub struct Settings {
    pub database: DatabaseSettings,
    pub telemetry: TelemetrySettings,
    pub server: ServerSettings,
}

pub struct DatabaseSettings {
    pub url: SecretString,  // SQLite connection string
}

pub struct TelemetrySettings {
    pub otlp_endpoint: Option<String>,
    pub sampling_ratio: f64,
}

pub struct ServerSettings {
    pub host: String,
    pub port: u16,
}
```

---

## Directory Structure

```
brio-core/
├── Cargo.toml              # Workspace manifest
├── Cargo.lock
├── deny.toml               # Cargo deny config
├── wit/                    # WIT interface definitions
│   ├── host.wit            # sql-state, session-fs
│   ├── mesh.wit            # service-mesh
│   ├── tools.wit           # tool-grep, tool-read-file
│   ├── logging.wit         # wasi:logging
│   └── brio.wit            # World definitions
├── kernel/                 # Rust host implementation
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs         # Entry point
│       ├── lib.rs          # Library exports
│       ├── host.rs         # BrioHostState
│       ├── engine/         # WASM runtime
│       │   ├── mod.rs
│       │   ├── linker.rs   # WIT binding setup
│       │   └── runtime.rs
│       ├── inference/      # LLM abstraction
│       │   ├── mod.rs
│       │   ├── provider.rs # Trait definition
│       │   ├── openai.rs   # OpenAI implementation
│       │   └── types.rs
│       ├── mesh/           # Service mesh IPC
│       │   └── mod.rs
│       ├── store/          # SQLite access
│       │   ├── mod.rs
│       │   ├── impl.rs
│       │   └── policy.rs
│       ├── vfs/            # Virtual filesystem
│       │   ├── mod.rs
│       │   ├── manager.rs
│       │   ├── diff.rs
│       │   └── reflink.rs
│       ├── ws/             # WebSocket
│       │   ├── mod.rs
│       │   ├── broadcaster.rs
│       │   ├── connection.rs
│       │   ├── handler.rs
│       │   └── types.rs
│       └── infrastructure/ # Cross-cutting concerns
│           ├── mod.rs
│           ├── audit.rs
│           ├── config.rs
│           ├── server.rs
│           └── telemetry.rs
└── components/             # WASM components
    ├── supervisor/         # Policy engine
    ├── agents/             # Stateful workers
    └── tools/              # Stateless functions
```

---

## Security Model

### Capability-Based Access

Components only receive capabilities explicitly granted:

| Component  | Capabilities                            |
| ---------- | --------------------------------------- |
| Supervisor | `sql-state`, `service-mesh`, `logging`  |
| Agent      | `session-fs`, `service-mesh`, `logging` |
| Tool       | `session-fs` (read-only), `logging`     |

### VFS Isolation

- Agents cannot access real filesystem directly
- All writes go to session sandbox
- Commit requires explicit host approval

### SQL Policy Enforcement

- Each component has a scope
- Queries validated against scope before execution
- Prevents cross-scope data access

---

## Telemetry

**Stack:** OpenTelemetry with optional OTLP export

**Traces:** All major operations instrumented with `#[instrument]`

**Metrics:** 
- Request latency histograms
- Active session count
- WebSocket client count
- Inference token usage

**Audit Log:** Critical events logged to structured audit trail

```rust
pub enum AuditEvent {
    SystemStartup { component: String },
    SystemShutdown { reason: String },
    SessionCreated { session_id: String, base_path: String },
    SessionCommitted { session_id: String },
    InferenceRequest { model: String, tokens: u32 },
}
```

---

## Future Considerations

1. ~~**Distributed Mesh**: Multi-node service mesh for horizontal scaling~~ ✅ **Implemented**
2. **Component Hot-Reload**: Update components without kernel restart
3. **Persistent Sessions**: Resume sessions across kernel restarts
4. **Plugin System**: Third-party tool/agent installation
5. ~~**Multi-Model Support**: Concurrent use of different LLM providers~~ ✅ **Implemented**

---

## Distributed Mesh

Brio now supports a multi-node service mesh using gRPC transport. Nodes can route calls transparently to local or remote components.

**Architecture:**
- **Local Routing**: `MeshRouter` (HashMap) for same-process components.
- **Remote Routing**: `RemoteRouter` (gRPC Client) for cross-node calls.
- **Node Registry**: Tracks `NodeId` mapped to `NodeAddress`.
- **Transport**: gRPC via `tonic` and `prost`.

**Configuration:**
Enable distributed mode by setting environment variables:
- `BRIO_NODE_ID`: Unique ID for the node (e.g., `node-1`)
- `BRIO_MESH_PORT`: Port to listen on (default `50051`)

**Usage:**
```rust
// Start in distributed mode
let node_id = NodeId::from("node-1");
let host = BrioHostState::new_distributed(db_url, registry, node_id).await?;

// Calls to "local_component" stay local
// Calls to "other_node/remote_component" are routed via gRPC
```

---

## Multi-Model Support

The kernel now supports concurrent use of multiple LLM providers through the `ProviderRegistry`:

```rust
// Register multiple providers
let registry = ProviderRegistry::new();
registry.register("openai", OpenAIProvider::new(openai_config));
registry.register("anthropic", AnthropicProvider::new(anthropic_config));
registry.set_default("openai");

// Access providers by name
let openai = host.inference_by_name("openai");
let anthropic = host.inference_by_name("anthropic");

// Or use the default provider
let default = host.inference();
```

**Supported Providers:**
- **OpenAI** (`OpenAIProvider`) - Compatible with OpenAI API and OpenRouter
- **Anthropic** (`AnthropicProvider`) - Claude models via Anthropic API

