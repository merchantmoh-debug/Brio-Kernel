# Brio TUI Development Guide

This document provides comprehensive documentation for building a Terminal User Interface (TUI) for Brio, similar to Claude Code. A TUI would serve as a local, interactive interface for working with the Brio Kernel.

---

## Table of Contents

1. [Architecture Overview](#architecture-overview)
2. [Core Components to Integrate](#core-components-to-integrate)
3. [Communication Protocols](#communication-protocols)
4. [Data Models](#data-models)
5. [Key APIs](#key-apis)
6. [TUI Design Recommendations](#tui-design-recommendations)
7. [Implementation Roadmap](#implementation-roadmap)

---

## Architecture Overview

Brio is a headless micro-kernel that orchestrates AI agents using the WebAssembly Component Model (WASI Preview 2). The TUI will communicate with the kernel via WebSocket, receiving real-time state updates as JSON Patches.

```
┌─────────────────────────────────────────────────────────────────┐
│                         TUI Client                               │
│  ┌─────────────┐  ┌──────────────┐  ┌─────────────────────────┐ │
│  │ Input Panel │  │ Output Panel │  │ Status/Context Panel    │ │
│  │ (User cmds) │  │ (Agent msgs) │  │ (Session, Tasks, Files) │ │
│  └─────────────┘  └──────────────┘  └─────────────────────────┘ │
└────────────────────────────┬────────────────────────────────────┘
                             │ WebSocket (JSON Patches)
                             ▼
┌─────────────────────────────────────────────────────────────────┐
│                       Brio Kernel                                │
│  ┌────────────────┐  ┌────────────────┐  ┌────────────────────┐ │
│  │ BrioHostState  │  │ SessionManager │  │ Broadcaster        │ │
│  │ (Orchestrator) │  │ (VFS Sandbox)  │  │ (WS JSON Patches)  │ │
│  └────────────────┘  └────────────────┘  └────────────────────┘ │
│  ┌────────────────┐  ┌────────────────┐  ┌────────────────────┐ │
│  │ LLMProvider    │  │ SqlStore       │  │ Service Mesh       │ │
│  │ (Inference)    │  │ (State DB)     │  │ (Component IPC)    │ │
│  └────────────────┘  └────────────────┘  └────────────────────┘ │
└─────────────────────────────────────────────────────────────────┘
```

### Key Concepts

| Concept             | Description                                                          |
| ------------------- | -------------------------------------------------------------------- |
| **Headless Kernel** | Brio has no UI; all interaction is via WebSocket                     |
| **JSON Patches**    | Real-time state updates using RFC 6902 format                        |
| **VFS Sandbox**     | File changes are made in temp directories, then atomically committed |
| **Service Mesh**    | Internal IPC between agents, tools, and supervisor                   |
| **WASI Components** | Agents/Tools run as isolated WebAssembly modules                     |

---

## Core Components to Integrate

### 1. WebSocket Broadcaster

**Location**: `kernel/src/ws/broadcaster.rs`

The `Broadcaster` manages WebSocket subscriptions and distributes state changes to all connected clients.

```rust
pub struct Broadcaster {
    sender: broadcast::Sender<BroadcastMessage>,
    client_count: Arc<AtomicUsize>,
}

impl Broadcaster {
    pub fn new() -> Self;
    pub fn subscribe(&self) -> BroadcastReceiver;
    pub fn broadcast(&self, message: BroadcastMessage) -> Result<(), WsError>;
    pub fn client_count(&self) -> usize;
}
```

**TUI Integration**: Connect to the WebSocket server and subscribe to receive `BroadcastMessage` events.

### 2. Broadcast Message Types

**Location**: `kernel/src/ws/types.rs`

```rust
pub enum BroadcastMessage {
    Patch(WsPatch),    // JSON Patch for state updates
    Shutdown,          // Server shutdown signal
}

pub struct WsPatch {
    pub path: String,           // JSON Pointer path (e.g., "/tasks/0/status")
    pub op: PatchOperation,     // add, remove, replace
    pub value: serde_json::Value,
}

pub enum PatchOperation {
    Add,
    Remove,
    Replace,
}
```

### 3. BrioHostState (Main Orchestrator)

**Location**: `kernel/src/host.rs`

The central state manager that coordinates all subsystems:

```rust
pub struct BrioHostState {
    mesh_router: RwLock<HashMap<String, Sender<MeshMessage>>>,
    db_pool: SqlitePool,
    broadcaster: Broadcaster,
    session_manager: Mutex<SessionManager>,
    inference_provider: Arc<Box<dyn LLMProvider>>,
}

impl BrioHostState {
    pub async fn new(db_url: &str, provider: Box<dyn LLMProvider>) -> Result<Self>;
    pub fn broadcaster(&self) -> &Broadcaster;
    pub fn broadcast_patch(&self, patch: WsPatch) -> Result<()>;
    pub async fn mesh_call(&self, target: &str, method: &str, payload: Payload) -> Result<Payload>;
    pub fn begin_session(&self, base_path: String) -> Result<String, String>;
    pub fn commit_session(&self, session_id: String) -> Result<(), String>;
    pub fn inference(&self) -> Arc<Box<dyn LLMProvider>>;
}
```

### 4. Session Manager (VFS Sandbox)

**Location**: `kernel/src/vfs/manager.rs`

Manages isolated workspaces for agent file operations:

```rust
pub struct SessionManager {
    sessions: HashMap<String, SessionInfo>,
    base_dir: PathBuf,  // Default: /tmp/brio
}

pub struct SessionInfo {
    base_path: PathBuf,         // Original project path
    session_path: PathBuf,      // Temp copy path
    base_hash: String,          // Snapshot hash for conflict detection
}

impl SessionManager {
    pub fn begin_session(&mut self, base_path: String) -> Result<String, String>;
    pub fn commit_session(&mut self, session_id: String) -> Result<(), String>;
    pub fn rollback_session(&mut self, session_id: String) -> Result<(), String>;
    pub fn get_session_path(&self, session_id: &str) -> Option<PathBuf>;
    pub fn active_session_count(&self) -> usize;
    pub fn cleanup_orphaned_sessions(&self) -> Result<usize, String>;
}
```

**TUI Usage**: Display active sessions, show session path, provide commit/rollback controls.

### 5. Inference Provider

**Location**: `kernel/src/inference/`

Abstraction for LLM API calls:

```rust
#[async_trait]
pub trait LLMProvider: Send + Sync {
    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse, InferenceError>;
}

pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<Message>,
}

pub struct ChatResponse {
    pub content: String,
    pub usage: Option<Usage>,
}

pub struct Message {
    pub role: Role,       // System, User, Assistant
    pub content: String,
}

pub enum InferenceError {
    ProviderError(String),
    RateLimit,
    ContextLengthExceeded,
    NetworkError(String),
    ConfigError(String),
}
```

### 6. SQL Store

**Location**: `kernel/src/store/impl.rs`

Policy-enforced SQLite access for task/state management:

```rust
pub struct SqlStore {
    pool: SqlitePool,
    policy: Box<dyn QueryPolicy>,
}

impl SqlStore {
    pub async fn query(&self, scope: &str, sql: &str, params: Vec<String>) 
        -> Result<Vec<GenericRow>, StoreError>;
    pub async fn execute(&self, scope: &str, sql: &str, params: Vec<String>) 
        -> Result<u32, StoreError>;
}

pub struct GenericRow {
    pub columns: Vec<String>,
    pub values: Vec<String>,
}
```

---

## Communication Protocols

### WebSocket Connection

The TUI connects to Brio's WebSocket server to:
1. **Receive** real-time state updates (JSON Patches)
2. **Send** user commands and requests

**Default Endpoint**: `ws://localhost:3000/ws`

### JSON Patch Format (RFC 6902)

State updates are sent as JSON Patches:

```json
{
  "type": "patch",
  "data": {
    "path": "/tasks/0/status",
    "op": "replace",
    "value": "completed"
  }
}
```

### User Input Protocol

User commands should be sent as JSON:

```json
{
  "type": "task",
  "content": "Fix the bug in auth.rs"
}
```

```json
{
  "type": "session",
  "action": "begin",
  "base_path": "./src"
}
```

```json
{
  "type": "session",
  "action": "commit",
  "session_id": "sess-abc123"
}
```

---

## Data Models

### Task Model

```rust
struct Task {
    id: String,
    content: String,
    status: TaskStatus,  // pending, in_progress, completed, failed
    priority: i32,
    created_at: DateTime,
    agent_id: Option<String>,
}
```

### Agent State

```rust
struct AgentState {
    id: String,
    name: String,
    status: AgentStatus,  // idle, working, error
    current_task: Option<String>,
    messages: Vec<Message>,
}
```

### File Change

```rust
struct FileChange {
    path: String,
    change_type: ChangeType,  // added, modified, deleted
    diff: Option<String>,
}
```

---

## Key APIs

### WIT Interface Definitions

These WIT interfaces define the contract between the kernel and components:

#### Service Mesh (`wit/mesh.wit`)

```wit
interface service-mesh {
    variant payload {
        json(string),
        binary(list<u8>)
    }
    call: func(target: string, method: string, args: payload) -> result<payload, string>;
}
```

#### SQL State (`wit/host.wit`)

```wit
interface sql-state {
    record row {
        columns: list<string>,
        values: list<string>
    }
    query: func(sql: string, params: list<string>) -> result<list<row>, string>;
    execute: func(sql: string, params: list<string>) -> result<u32, string>;
}
```

#### Session FS (`wit/host.wit`)

```wit
interface session-fs {
    begin-session: func(base-path: string) -> result<string, string>;
    commit-session: func(session-id: string) -> result<tuple<>, string>;
}
```

#### Tools (`wit/tools.wit`)

```wit
interface tool-grep {
    record grep-match { line-number: u32, content: string }
    record grep-result { file-path: string, matches: list<grep-match> }
    grep: func(pattern: string, path: string) -> result<list<grep-result>, string>;
}

interface tool-read-file {
    read-file: func(path: string) -> result<string, string>;
    read-file-range: func(path: string, start: u32, end: u32) -> result<string, string>;
}
```

---

## TUI Design Recommendations

### Recommended Technology Stack

| Component            | Recommendation                                    | Rationale                                |
| -------------------- | ------------------------------------------------- | ---------------------------------------- |
| **Language**         | Rust                                              | Seamless integration with Brio Kernel    |
| **TUI Framework**    | [Ratatui](https://github.com/ratatui-org/ratatui) | Modern, well-maintained, great ecosystem |
| **Async Runtime**    | Tokio                                             | Already used by Brio Kernel              |
| **WebSocket Client** | tokio-tungstenite                                 | Native async WebSocket                   |
| **JSON Handling**    | serde_json                                        | Standard, already used                   |

### UI Layout (Claude Code-inspired)

```
┌─────────────────────────────────────────────────────────────────────────┐
│ Brio TUI                                                    Session: X  │
├─────────────────────────────────────────────────────────────────────────┤
│ ┌─────────────────────────────────────┐ ┌─────────────────────────────┐ │
│ │ Conversation                        │ │ Context                     │ │
│ │                                     │ │                             │ │
│ │ [User] Fix the auth bug...          │ │ 📁 Active Session:          │ │
│ │                                     │ │    ./src                    │ │
│ │ [Agent] I'll analyze the auth       │ │                             │ │
│ │ module. Let me search for...        │ │ 📄 Modified Files:          │ │
│ │                                     │ │    auth.rs (+15, -3)        │ │
│ │ [Tool: grep] Found 3 matches...     │ │    config.rs (+2, -0)       │ │
│ │                                     │ │                             │ │
│ │ [Agent] I found the issue. The      │ │ 📊 Token Usage:             │ │
│ │ problem is in validate_token()...   │ │    Prompt: 1,234            │ │
│ │                                     │ │    Response: 567            │ │
│ │                                     │ │                             │ │
│ └─────────────────────────────────────┘ └─────────────────────────────┘ │
├─────────────────────────────────────────────────────────────────────────┤
│ > Type your message...                             [Ctrl+C] Exit        │
└─────────────────────────────────────────────────────────────────────────┘
```

### Key UI Components

1. **Conversation Panel**
   - Display messages from user, agent, and tool outputs
   - Support syntax highlighting for code blocks
   - Show thinking/loading indicators

2. **Context Panel**
   - Active session status
   - Modified files with diff summary
   - Token usage tracking
   - Current task status

3. **Input Area**
   - Multi-line input support
   - History navigation (↑/↓)
   - Tab completion for commands

4. **Status Bar**
   - Connection status (WebSocket)
   - Current model
   - Session ID
   - Keybindings help

### Keybindings

| Key          | Action             |
| ------------ | ------------------ |
| `Ctrl+Enter` | Send message       |
| `Ctrl+C`     | Exit / Cancel      |
| `Ctrl+S`     | Commit session     |
| `Ctrl+R`     | Rollback session   |
| `Ctrl+N`     | New session        |
| `Tab`        | Autocomplete       |
| `↑/↓`        | History navigation |
| `Ctrl+L`     | Clear screen       |
| `Ctrl+D`     | Toggle debug panel |

---

## Implementation Roadmap

### Phase 1: Foundation (Week 1-2)

1. **WebSocket Client**
   - Connect to Brio Kernel WebSocket server
   - Handle reconnection logic
   - Parse incoming JSON Patches
   - Maintain local state synchronized with patches

2. **Basic TUI Layout**
   - Set up Ratatui with Tokio
   - Create split-pane layout
   - Implement scrollable message list
   - Add input field with editing

3. **State Management**
   - Create local state store
   - Apply JSON Patches to update state
   - Implement state diffing for UI updates

### Phase 2: Core Features (Week 3-4)

1. **Message Display**
   - Render user/agent/tool messages
   - Syntax highlighting for code
   - Diff visualization

2. **Command Interface**
   - Parse user input
   - Send commands via WebSocket
   - Handle command responses

3. **Session Controls**
   - Begin/commit/rollback session
   - Display session status
   - Show modified files

### Phase 3: Polish (Week 5-6)

1. **UX Improvements**
   - Loading indicators
   - Error handling with friendly messages
   - Keyboard shortcuts
   - Help panel

2. **Advanced Features**
   - File browser integration
   - Diff viewer
   - Token usage tracking
   - Command history persistence

3. **Configuration**
   - Config file support
   - Theme customization
   - Keybinding customization

---

## Example Code Snippets

### WebSocket Client Setup

```rust
use tokio_tungstenite::{connect_async, tungstenite::Message};
use futures_util::{SinkExt, StreamExt};

async fn connect_to_brio() -> Result<(), Box<dyn std::error::Error>> {
    let (ws_stream, _) = connect_async("ws://localhost:3000/ws").await?;
    let (mut write, mut read) = ws_stream.split();
    
    // Receive messages
    while let Some(msg) = read.next().await {
        match msg? {
            Message::Text(text) => {
                let patch: BroadcastMessage = serde_json::from_str(&text)?;
                handle_patch(patch).await;
            }
            Message::Close(_) => break,
            _ => {}
        }
    }
    
    Ok(())
}
```

### JSON Patch Application

```rust
use json_patch::patch;
use serde_json::Value;

fn apply_patch(state: &mut Value, ws_patch: &WsPatch) {
    let patch_doc = json_patch::Patch(vec![
        match ws_patch.op {
            PatchOperation::Add => 
                json_patch::PatchOperation::Add { 
                    path: ws_patch.path.clone(), 
                    value: ws_patch.value.clone() 
                },
            PatchOperation::Replace => 
                json_patch::PatchOperation::Replace { 
                    path: ws_patch.path.clone(), 
                    value: ws_patch.value.clone() 
                },
            PatchOperation::Remove => 
                json_patch::PatchOperation::Remove { 
                    path: ws_patch.path.clone() 
                },
        }
    ]);
    
    patch(state, &patch_doc).expect("Failed to apply patch");
}
```

### Ratatui App Structure

```rust
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    widgets::{Block, Borders, Paragraph},
    Terminal,
};

struct App {
    messages: Vec<Message>,
    input: String,
    session: Option<SessionInfo>,
    state: serde_json::Value,
}

fn ui(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
        .split(f.size());
    
    // Left: Conversation
    let conversation = Paragraph::new(render_messages(&app.messages))
        .block(Block::default().title("Conversation").borders(Borders::ALL));
    f.render_widget(conversation, chunks[0]);
    
    // Right: Context
    let context = Paragraph::new(render_context(&app.session))
        .block(Block::default().title("Context").borders(Borders::ALL));
    f.render_widget(context, chunks[1]);
}
```

---

## Configuration Schema

```toml
# brio-tui.toml

[connection]
host = "localhost"
port = 3000
reconnect_interval_ms = 5000

[ui]
theme = "dark"  # dark, light, custom
show_token_usage = true
show_timestamps = true
max_history = 1000

[keybindings]
send = "ctrl+enter"
exit = "ctrl+c"
commit = "ctrl+s"
rollback = "ctrl+r"

[inference]
default_model = "gpt-4"
```

---

## Testing Strategy

1. **Unit Tests**
   - JSON Patch application
   - State management
   - Message parsing

2. **Integration Tests**
   - WebSocket connection handling
   - Full message round-trip
   - Session lifecycle

3. **Manual Testing**
   - UI responsiveness
   - Keyboard navigation
   - Error recovery

---

## Dependencies

```toml
[dependencies]
ratatui = "0.26"
crossterm = "0.27"
tokio = { version = "1", features = ["full"] }
tokio-tungstenite = "0.21"
futures-util = "0.3"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
json-patch = "1"
tracing = "0.1"
tracing-subscriber = "0.3"
```

---

## Resources

- [Ratatui Documentation](https://ratatui.rs/)
- [JSON Patch RFC 6902](https://tools.ietf.org/html/rfc6902)
- [Tokio-Tungstenite](https://github.com/snapview/tokio-tungstenite)
- [Claude Code Architecture](https://docs.anthropic.com/claude/docs/claude-code) (for inspiration)

---

*Last Updated: 2026-01-21*
