# Brio Kernel API Reference

Complete API documentation for integrating with the Brio Kernel.

---

## Table of Contents

1. [WebSocket API](#websocket-api)
2. [WIT Interfaces](#wit-interfaces)
3. [Rust Public API](#rust-public-api)
4. [Data Types](#data-types)
5. [Error Codes](#error-codes)

---

## WebSocket API

### Connection

**Endpoint:** `ws://{host}:{port}/ws`

**Default:** `ws://localhost:3000/ws`

### Message Format

All messages are JSON-encoded.

#### Incoming (Server → Client)

```typescript
// JSON Patch message
{
  "type": "patch",
  "data": {
    "path": string,     // JSON Pointer (RFC 6901)
    "op": "add" | "remove" | "replace",
    "value": any        // Omitted for "remove"
  }
}

// Shutdown notification
{
  "type": "shutdown"
}
```

#### Outgoing (Client → Server)

```typescript
// Task submission
{
  "type": "task",
  "content": string,
  "priority"?: number   // Default: 0
}

// Session control
{
  "type": "session",
  "action": "begin" | "commit" | "rollback",
  "base_path"?: string,   // Required for "begin"
  "session_id"?: string   // Required for "commit"/"rollback"
}

// Query state
{
  "type": "query",
  "sql": string,
  "params"?: string[]
}
```

### Example Session

```javascript
const ws = new WebSocket("ws://localhost:3000/ws");

ws.onmessage = (event) => {
  const msg = JSON.parse(event.data);
  if (msg.type === "patch") {
    applyPatch(state, msg.data);
  }
};

// Submit a task
ws.send(JSON.stringify({
  type: "task",
  content: "Fix the authentication bug"
}));
```

---

## WIT Interfaces

### service-mesh

Component-to-component communication.

```wit
package brio:core;

interface service-mesh {
    variant payload {
        json(string),
        binary(list<u8>)
    }

    /// Call another component
    /// target: Component ID (e.g., "tool_grep", "agent_coder")
    /// method: Method name to invoke
    /// args: Request payload
    call: func(target: string, method: string, args: payload) 
        -> result<payload, string>;
}
```

**Example Usage (from component):**
```rust
let result = service_mesh::call(
    "tool_grep",
    "search",
    Payload::Json(r#"{"pattern": "TODO", "path": "."}"#.into())
)?;
```

---

### sql-state

SQLite database access.

```wit
package brio:core;

interface sql-state {
    record row {
        columns: list<string>,
        values: list<string>
    }

    /// Execute SELECT query
    query: func(sql: string, params: list<string>) 
        -> result<list<row>, string>;

    /// Execute INSERT/UPDATE/DELETE
    /// Returns number of affected rows
    execute: func(sql: string, params: list<string>) 
        -> result<u32, string>;
}
```

**Example Usage:**
```rust
// Query
let rows = sql_state::query(
    "SELECT * FROM tasks WHERE status = ?",
    vec!["pending".into()]
)?;

// Insert
let affected = sql_state::execute(
    "INSERT INTO tasks (content, status) VALUES (?, ?)",
    vec!["Fix bug".into(), "pending".into()]
)?;
```

---

### session-fs

Filesystem sandbox management.

```wit
package brio:core;

interface session-fs {
    /// Create sandboxed copy of directory
    /// Returns: session_id
    begin-session: func(base-path: string) -> result<string, string>;

    /// Commit changes from sandbox to original
    commit-session: func(session-id: string) -> result<tuple<>, string>;
}
```

**Lifecycle:**
1. `begin-session("./src")` → Creates `/tmp/brio/sess-{uuid}`, returns ID
2. Agent works in sandbox directory
3. `commit-session(session_id)` → Applies changes atomically

---

### tool-grep

File search functionality.

```wit
package brio:tools;

interface tool-grep {
    record grep-match {
        line-number: u32,
        content: string,
    }

    record grep-result {
        file-path: string,
        matches: list<grep-match>,
    }

    /// Search for pattern in files
    grep: func(pattern: string, path: string) 
        -> result<list<grep-result>, string>;
}
```

---

### tool-read-file

File reading functionality.

```wit
package brio:tools;

interface tool-read-file {
    /// Read entire file
    read-file: func(path: string) -> result<string, string>;

    /// Read specific line range (1-indexed, inclusive)
    read-file-range: func(path: string, start-line: u32, end-line: u32) 
        -> result<string, string>;
}
```

---

## Rust Public API

### BrioHostState

```rust
pub struct BrioHostState { /* ... */ }

impl BrioHostState {
    /// Create new host state
    pub async fn new(
        db_url: &str, 
        provider: Box<dyn LLMProvider>
    ) -> Result<Self>;

    /// Register a component for mesh routing
    pub fn register_component(
        &self, 
        id: String, 
        sender: Sender<MeshMessage>
    );

    /// Get database pool reference
    pub fn db(&self) -> &SqlitePool;

    /// Get store with policy for given scope
    pub fn get_store(&self, scope: &str) -> SqlStore;

    /// Get broadcaster reference
    pub fn broadcaster(&self) -> &Broadcaster;

    /// Broadcast a state patch
    pub fn broadcast_patch(&self, patch: WsPatch) -> Result<()>;

    /// Call component via service mesh
    pub async fn mesh_call(
        &self, 
        target: &str, 
        method: &str, 
        payload: Payload
    ) -> Result<Payload>;

    /// Begin VFS session
    pub fn begin_session(&self, base_path: String) -> Result<String, String>;

    /// Commit VFS session
    pub fn commit_session(&self, session_id: String) -> Result<(), String>;

    /// Get LLM provider
    pub fn inference(&self) -> Arc<Box<dyn LLMProvider>>;
}
```

---

### SessionManager

```rust
pub struct SessionManager { /* ... */ }

impl SessionManager {
    /// Create new session manager
    pub fn new() -> Self;

    /// Begin session with base directory copy
    pub fn begin_session(&mut self, base_path: String) -> Result<String, String>;

    /// Commit session changes atomically
    pub fn commit_session(&mut self, session_id: String) -> Result<(), String>;

    /// Rollback session (discard changes)
    pub fn rollback_session(&mut self, session_id: String) -> Result<(), String>;

    /// Get path to session working directory
    pub fn get_session_path(&self, session_id: &str) -> Option<PathBuf>;

    /// Get count of active sessions
    pub fn active_session_count(&self) -> usize;

    /// Cleanup orphaned session directories
    pub fn cleanup_orphaned_sessions(&self) -> Result<usize, String>;
}
```

---

### LLMProvider Trait

```rust
#[async_trait]
pub trait LLMProvider: Send + Sync {
    /// Execute chat completion
    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse, InferenceError>;
}
```

---

### Broadcaster

```rust
pub struct Broadcaster { /* ... */ }

impl Broadcaster {
    /// Create new broadcaster
    pub fn new() -> Self;

    /// Subscribe to broadcasts
    pub fn subscribe(&self) -> BroadcastReceiver;

    /// Send message to all subscribers
    pub fn broadcast(&self, message: BroadcastMessage) -> Result<(), WsError>;

    /// Get current subscriber count
    pub fn client_count(&self) -> usize;
}
```

---

### SqlStore

```rust
pub struct SqlStore { /* ... */ }

impl SqlStore {
    /// Create store with pool and policy
    pub fn new(pool: SqlitePool, policy: Box<dyn QueryPolicy>) -> Self;

    /// Execute SELECT query with policy check
    pub async fn query(
        &self,
        scope: &str,
        sql: &str,
        params: Vec<String>,
    ) -> Result<Vec<GenericRow>, StoreError>;

    /// Execute INSERT/UPDATE/DELETE with policy check
    pub async fn execute(
        &self,
        scope: &str,
        sql: &str,
        params: Vec<String>,
    ) -> Result<u32, StoreError>;
}
```

---

## Data Types

### Message Types

```rust
pub struct Message {
    pub role: Role,
    pub content: String,
}

pub enum Role {
    System,
    User,
    Assistant,
}
```

### Chat Types

```rust
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<Message>,
}

pub struct ChatResponse {
    pub content: String,
    pub usage: Option<Usage>,
}

pub struct Usage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}
```

### Mesh Types

```rust
pub struct MeshMessage {
    pub target: String,
    pub method: String,
    pub payload: Payload,
    pub reply_tx: oneshot::Sender<Result<Payload, String>>,
}

pub enum Payload {
    Json(String),
    Binary(Vec<u8>),
}
```

### WebSocket Types

```rust
pub enum BroadcastMessage {
    Patch(WsPatch),
    Shutdown,
}

pub struct WsPatch {
    pub path: String,
    pub op: PatchOperation,
    pub value: serde_json::Value,
}

pub enum PatchOperation {
    Add,
    Remove,
    Replace,
}
```

### Store Types

```rust
pub struct GenericRow {
    pub columns: Vec<String>,
    pub values: Vec<String>,
}
```

---

## Error Codes

### InferenceError

| Variant                 | Description              | Retryable          |
| ----------------------- | ------------------------ | ------------------ |
| `ProviderError`         | Generic provider failure | No                 |
| `RateLimit`             | API rate limit exceeded  | Yes (with backoff) |
| `ContextLengthExceeded` | Request too large        | No                 |
| `NetworkError`          | Connection failure       | Yes                |
| `ConfigError`           | Invalid configuration    | No                 |

### StoreError

| Variant       | Description               |
| ------------- | ------------------------- |
| `DbError`     | SQLite error              |
| `PolicyError` | Query policy violation    |
| `Internal`    | Internal processing error |

### WsError

| Variant              | Description                 |
| -------------------- | --------------------------- |
| `ChannelClosed`      | Broadcast channel closed    |
| `SerializationError` | JSON serialization failed   |
| `ConnectionError`    | WebSocket connection failed |

---

## HTTP Endpoints

*(Future implementation)*

| Method   | Endpoint                       | Description          |
| -------- | ------------------------------ | -------------------- |
| `GET`    | `/health`                      | Health check         |
| `GET`    | `/metrics`                     | Prometheus metrics   |
| `GET`    | `/api/v1/sessions`             | List active sessions |
| `POST`   | `/api/v1/sessions`             | Begin session        |
| `DELETE` | `/api/v1/sessions/{id}`        | Rollback session     |
| `POST`   | `/api/v1/sessions/{id}/commit` | Commit session       |
