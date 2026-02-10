# TUI Client Integration Guide

This guide provides the architecture specification and implementation patterns for building Terminal User Interface (TUI) clients that integrate with Brio-Kernel. A TUI client serves as an interactive, local-first interface for AI-assisted development workflows.

---

## Architecture Overview

Brio-Kernel is a strictly headless micro-kernel. All user interaction happens through WebSocket connections, enabling multiple client types (TUI, Web UI, IDE plugins) to connect simultaneously.

```
┌─────────────────────────────────────────────────────────────────┐
│                        TUI Client                                │
│  ┌─────────────────┐  ┌──────────────────┐  ┌─────────────────┐ │
│  │ Conversation    │  │ Context Panel    │  │ Input Area      │ │
│  │ (Chat history)  │  │ (Files, Session) │  │ (User commands) │ │
│  └─────────────────┘  └──────────────────┘  └─────────────────┘ │
└──────────────────────────┬──────────────────────────────────────┘
                           │ WebSocket (ws://localhost:3000/ws)
                           ▼
┌─────────────────────────────────────────────────────────────────┐
│                      Brio Kernel                                 │
│  ┌────────────────┐  ┌────────────────┐  ┌────────────────────┐ │
│  │ BrioHostState  │  │ SessionManager │  │ WebSocket          │ │
│  │ (Orchestrator) │  │ (VFS Sandbox)  │  │ Broadcaster        │ │
│  └────────────────┘  └────────────────┘  └────────────────────┘ │
└─────────────────────────────────────────────────────────────────┘
```

### Key Principles

| Principle | Description |
|-----------|-------------|
| **Headless Kernel** | The kernel has no built-in UI; all interaction is API-driven |
| **Real-time Sync** | Clients receive incremental state updates via JSON Patch |
| **Multi-Client** | Multiple TUI clients can connect to the same kernel instance |
| **Stateless UI** | TUI maintains local state, synced via patches from kernel |

---

## Communication Protocol

### WebSocket Connection

TUI clients connect to the kernel via WebSocket for bidirectional real-time communication:

**Endpoint**: `ws://localhost:3000/ws` (configurable)

```
┌─────────┐      WebSocket       ┌─────────┐
│   TUI   │ ◄──────────────────► │  Kernel │
│ Client  │   JSON messages      │ Server  │
└─────────┘                      └─────────┘
```

### Message Types

The protocol supports four primary message categories:

| Type | Direction | Purpose |
|------|-----------|---------|
| `task` | TUI → Kernel | Submit user requests to agents |
| `session` | TUI ↔ Kernel | Manage VFS sessions (begin/commit/rollback) |
| `query` | TUI → Kernel | Request current state snapshots |
| `patch` | Kernel → TUI | Receive incremental state updates |

### Message Format

**Task Submission** (TUI → Kernel):
```json
{
  "type": "task",
  "content": "Refactor the authentication module"
}
```

**Session Control** (TUI → Kernel):
```json
{
  "type": "session",
  "action": "begin",
  "base_path": "./src"
}
```

**State Query** (TUI → Kernel):
```json
{
  "type": "query",
  "path": "/tasks"
}
```

**State Update** (Kernel → TUI):
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

### Connection Lifecycle

```
         ┌──────────┐
         │  Closed  │
         └────┬─────┘
              │ connect()
              ▼
    ┌──────────────────┐
    │    Connecting    │ ─────► Retry with backoff (max 5 attempts)
    └────────┬─────────┘
             │ WebSocket open
             ▼
    ┌──────────────────┐
    │     Active       │ ◄───── Send/Receive messages
    └────────┬─────────┘
             │ Connection drop
             ▼
    ┌──────────────────┐
    │  Reconnecting    │ ─────► Exponential backoff (1s, 2s, 4s, 8s)
    └────────┬─────────┘
             │ Max retries exceeded
             ▼
         ┌──────────┐
         │  Failed  │
         └──────────┘
```

---

## State Synchronization

### JSON Patch Protocol (RFC 6902)

Brio-Kernel uses JSON Patch for efficient, incremental state updates. Instead of sending full state snapshots, the kernel computes and broadcasts only the changes.

**Patch Operations**:

| Operation | Description | Example |
|-----------|-------------|---------|
| `add` | Insert new value | Add a new task to the list |
| `remove` | Delete existing value | Remove a completed task |
| `replace` | Update existing value | Change task status |

**Example Patch Sequence**:

```json
// Task created
{ "op": "add", "path": "/tasks/-", "value": { "id": "t1", "content": "Fix bug", "status": "pending" }}

// Task status updated
{ "op": "replace", "path": "/tasks/0/status", "value": "in_progress" }

// Agent assigned
{ "op": "replace", "path": "/tasks/0/agent_id", "value": "coder-001" }

// Task completed
{ "op": "replace", "path": "/tasks/0/status", "value": "completed" }
```

### Incremental Updates vs Full Refresh

**When to Use Patches**:
- Real-time task status changes
- Agent state updates
- File modification tracking
- Session state changes

**When to Request Full State**:
- Initial connection
- After reconnection
- On explicit refresh command
- When patch sequence is corrupted

### State Consistency Model

The TUI client maintains a local copy of the kernel state and applies patches atomically:

```rust
struct AppState {
    tasks: Vec<Task>,
    agents: Vec<Agent>,
    session: Option<Session>,
    version: u64,  // Monotonic counter for ordering
}

fn apply_patch(state: &mut AppState, patch: WsPatch) -> Result<(), Error> {
    // Apply patch atomically
    json_patch::patch(&mut state.tasks, &patch)?;
    state.version += 1;
    Ok(())
}
```

### Handling Out-of-Order Updates

In rare cases, patches may arrive out of order. Strategies for handling:

1. **Version Tracking**: Include sequence numbers in patches
2. **Buffering**: Hold out-of-order patches until missing ones arrive
3. **Full Sync**: Request complete state if gap detected
4. **Idempotent Operations**: Design patches to be safely reapplied

---

## TUI Architecture

### Recommended Stack

| Component | Recommendation | Rationale |
|-----------|----------------|-----------|
| **Language** | Rust | Native integration with kernel, performance |
| **TUI Framework** | [Ratatui](https://ratatui.rs/) | Modern, async-friendly, extensive widgets |
| **Async Runtime** | Tokio | Matches kernel runtime, WebSocket support |
| **WebSocket Client** | tokio-tungstenite | Native async, compatible with Tokio |
| **JSON Handling** | serde + json-patch | Standard ecosystem, RFC 6902 support |

### Component Structure

```
┌─────────────────────────────────────────────┐
│                App                          │
│  ┌─────────────┐  ┌──────────────────────┐  │
│  │  UI Loop    │  │   WebSocket Handler  │  │
│  │  (Ratatui)  │  │   (tokio-tungstenite)│  │
│  └──────┬──────┘  └──────────┬───────────┘  │
│         │                    │              │
│         ▼                    ▼              │
│  ┌──────────────────────────────────────┐   │
│  │           State Manager               │   │
│  │  ┌─────────────┐  ┌────────────────┐ │   │
│  │  │ Local State │  │ Patch Applier  │ │   │
│  │  └─────────────┘  └────────────────┘ │   │
│  └──────────────────────────────────────┘   │
└─────────────────────────────────────────────┘
```

### Event Loop Design

The TUI uses a dual-event-loop architecture:

**Terminal Event Loop** (main thread):
```rust
loop {
    // Poll terminal events with timeout
    if event::poll(Duration::from_millis(100))? {
        match event::read()? {
            Event::Key(key) => handle_key_event(key).await,
            Event::Resize(_, _) => redraw(),
            _ => {}
        }
    }
    
    // Check for state updates
    if let Ok(patch) = patch_rx.try_recv() {
        state.apply_patch(patch);
        redraw();
    }
}
```

**WebSocket Event Loop** (Tokio task):
```rust
tokio::spawn(async move {
    while let Some(msg) = ws_stream.next().await {
        match msg {
            Ok(Message::Text(text)) => {
                let patch: BroadcastMessage = serde_json::from_str(&text)?;
                patch_tx.send(patch).await?;
            }
            Ok(Message::Close(_)) => handle_disconnect().await,
            Err(e) => handle_error(e).await,
        }
    }
});
```

### State Management Approach

**Immutable State with Selective Updates**:
```rust
#[derive(Clone)]
struct AppState {
    tasks: Arc<RwLock<Vec<Task>>>,
    messages: Arc<RwLock<Vec<Message>>>,
    session: Arc<RwLock<Option<Session>>>,
}

impl AppState {
    fn apply_patch(&self, patch: WsPatch) {
        let mut tasks = self.tasks.write().unwrap();
        json_patch::patch(&mut *tasks, &patch).unwrap();
    }
}
```

---

## Key Interactions

### Connecting to Kernel

```rust
use tokio_tungstenite::connect_async;
use futures_util::{SinkExt, StreamExt};

async fn connect_to_kernel(url: &str) -> Result<WebSocketStream, Error> {
    let (ws_stream, _) = connect_async(url).await?;
    
    // Request full state on connection
    let init_msg = json!({
        "type": "query",
        "path": "/"
    });
    
    ws_stream.send(Message::Text(init_msg.to_string())).await?;
    Ok(ws_stream)
}
```

### Submitting Tasks

```rust
async fn submit_task(
    ws: &mut WebSocketStream,
    content: &str
) -> Result<(), Error> {
    let msg = json!({
        "type": "task",
        "content": content
    });
    
    ws.send(Message::Text(msg.to_string())).await?;
    Ok(())
}
```

### Receiving Real-Time Updates

```rust
async fn handle_incoming_messages(
    mut ws: WebSocketStream,
    state: Arc<AppState>
) {
    while let Some(msg) = ws.next().await {
        match msg {
            Ok(Message::Text(text)) => {
                if let Ok(patch) = serde_json::from_str::<WsPatch>(&text) {
                    state.apply_patch(patch);
                }
            }
            _ => {}
        }
    }
}
```

### Session Control

**Begin Session**:
```rust
async fn begin_session(ws: &mut WebSocketStream, path: &str) {
    let msg = json!({
        "type": "session",
        "action": "begin",
        "base_path": path
    });
    ws.send(Message::Text(msg.to_string())).await.unwrap();
}
```

**Commit/Rollback**:
```rust
async fn end_session(ws: &mut WebSocketStream, action: &str, id: &str) {
    let msg = json!({
        "type": "session",
        "action": action,  // "commit" or "rollback"
        "session_id": id
    });
    ws.send(Message::Text(msg.to_string())).await.unwrap();
}
```

### Querying State

```rust
async fn query_state(ws: &mut WebSocketStream, path: &str) {
    let msg = json!({
        "type": "query",
        "path": path  // e.g., "/tasks", "/agents"
    });
    ws.send(Message::Text(msg.to_string())).await.unwrap();
}
```

---

## UI Layout Recommendations

### Recommended Layout

```
┌──────────────────────────────────────────────────────────────────────┐
│ Brio TUI v0.1.0                                       Session: abc12 │
├──────────────────────────────────────────────────────────────────────┤
│ ┌─────────────────────────────────────┐ ┌──────────────────────────┐ │
│ │ Conversation                        │ │ Context                  │ │
│ │                                     │ │                          │ │
│ │ [User] Fix the auth bug...          │ │ 📁 Session:              │ │
│ │                                     │ │    ./src                 │ │
│ │ [Agent] I'll analyze the code...    │ │                          │ │
│ │                                     │ │ 📄 Modified:             │ │
│ │ [Tool: grep] Found 3 matches...     │ │    auth.rs (+15/-3)      │ │
│ │                                     │ │    config.rs (+2/-0)     │ │
│ │ [Agent] The issue is in line 42...  │ │                          │ │
│ │                                     │ │ 📊 Token Usage:          │ │
│ │ [System] Changes committed          │ │    Prompt: 1,234         │ │
│ │                                     │ │    Response: 567         │ │
│ └─────────────────────────────────────┘ └──────────────────────────┘ │
├──────────────────────────────────────────────────────────────────────┤
│ > Type your message...                              [Ctrl+C] Exit    │
└──────────────────────────────────────────────────────────────────────┘
```

### Panel Descriptions

**1. Conversation Panel** (60-70% width)
- Scrollable message history
- User messages (right-aligned)
- Agent responses (left-aligned)
- Tool outputs (monospace, collapsible)
- System notifications

**2. Context Panel** (30-40% width)
- Active session info
- Modified files list with diff stats
- Current agent status
- Token usage metrics
- File tree (optional)

**3. Input Area** (fixed height)
- Multi-line text input
- Command history (↑/↓)
- Character counter
- Send button or hint

**4. Status Bar** (1 line)
- Connection status indicator
- Current session ID (truncated)
- Active keybindings hint

### Keybinding Schemes

**Default Keybindings**:

| Key | Action |
|-----|--------|
| `Ctrl+Enter` | Send message |
| `Ctrl+C` | Exit / Cancel current operation |
| `Ctrl+S` | Commit session |
| `Ctrl+R` | Rollback session |
| `Ctrl+N` | New session |
| `Tab` | Focus next panel / autocomplete |
| `↑/↓` | Navigate history |
| `Ctrl+L` | Clear screen |
| `Ctrl+D` | Toggle debug panel |
| `Esc` | Cancel input / Close modal |

**Vim Mode** (optional):
- `i` - Enter insert mode
- `Esc` - Normal mode
- `:q` - Quit
- `:w` - Commit session

---

## Error Handling

### Connection Drops

**Detection**:
- WebSocket close event
- Ping timeout (>30s)
- Failed message send

**Recovery Strategy**:
```rust
enum ConnectionState {
    Connected,
    Disconnected,
    Reconnecting { attempt: u32 },
    Failed,
}

async fn handle_disconnect(&mut self) {
    self.state = ConnectionState::Reconnecting { attempt: 1 };
    
    for attempt in 1..=MAX_RETRIES {
        match self.try_reconnect().await {
            Ok(()) => {
                self.state = ConnectionState::Connected;
                self.request_full_state().await;
                return;
            }
            Err(e) if attempt < MAX_RETRIES => {
                let delay = Duration::from_secs(2_u64.pow(attempt));
                sleep(delay).await;
            }
            Err(e) => {
                self.state = ConnectionState::Failed;
                self.show_error("Connection failed permanently");
            }
        }
    }
}
```

### Reconnection Strategies

| Strategy | Use Case | Implementation |
|----------|----------|----------------|
| **Exponential Backoff** | General reconnection | 1s, 2s, 4s, 8s delays |
| **Full State Sync** | After reconnection | Request `/` query |
| **Patch Replay** | Brief disconnects | Buffer patches, apply on reconnect |
| **User Prompt** | Extended offline | Ask user to retry or exit |

### Error Display Patterns

**Status Bar Indicators**:
- `● Connected` (green)
- `○ Reconnecting...` (yellow)
- `✕ Disconnected` (red)

**Toast Notifications**:
- Brief errors (3s auto-dismiss)
- Persistent errors (until acknowledged)
- Error details in modal on demand

**User Feedback**:
- Loading spinners during operations
- Progress indicators for long tasks
- Success confirmations for commits

---

## Example: Minimal TUI Client

### Complete Minimal Example

```rust
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    terminal::{disable_raw_mode, enable_raw_mode},
};
use futures_util::{SinkExt, StreamExt};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    widgets::{Block, Borders, Paragraph},
    Terminal,
};
use serde_json::json;
use std::io;
use tokio::sync::mpsc;
use tokio_tungstenite::{connect_async, tungstenite::Message};

struct App {
    messages: Vec<String>,
    input: String,
    ws_tx: mpsc::UnboundedSender<String>,
}

impl App {
    fn new(ws_tx: mpsc::UnboundedSender<String>) -> Self {
        Self {
            messages: vec!["Connected to Brio Kernel".to_string()],
            input: String::new(),
            ws_tx,
        }
    }

    fn on_key(&mut self, key: KeyCode) {
        match key {
            KeyCode::Enter => {
                let msg = self.input.clone();
                self.messages.push(format!("> {}", msg));
                let payload = json!({"type": "task", "content": msg}).to_string();
                let _ = self.ws_tx.send(payload);
                self.input.clear();
            }
            KeyCode::Char(c) => self.input.push(c),
            KeyCode::Backspace => { self.input.pop(); }
            _ => {}
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Setup WebSocket
    let (ws_stream, _) = connect_async("ws://localhost:3000/ws").await?;
    let (mut write, mut read) = ws_stream.split();
    
    let (ws_tx, mut ws_rx) = mpsc::unbounded_channel::<String>();
    
    // Spawn WebSocket writer task
    tokio::spawn(async move {
        while let Some(msg) = ws_rx.recv().await {
            let _ = write.send(Message::Text(msg)).await;
        }
    });
    
    // Setup terminal
    enable_raw_mode()?;
    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend)?;
    
    let mut app = App::new(ws_tx);
    
    // Spawn WebSocket reader task
    let ws_messages = app.messages.clone();
    tokio::spawn(async move {
        while let Some(Ok(Message::Text(text))) = read.next().await {
            // Handle incoming patches
            println!("Received: {}", text);
        }
    });
    
    // Main event loop
    loop {
        terminal.draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(0), Constraint::Length(3)])
                .split(f.size());
            
            let messages = app.messages.join("\n");
            let conversation = Paragraph::new(messages)
                .block(Block::default().title("Conversation").borders(Borders::ALL));
            f.render_widget(conversation, chunks[0]);
            
            let input = Paragraph::new(app.input.as_str())
                .block(Block::default().title("Input").borders(Borders::ALL));
            f.render_widget(input, chunks[1]);
        })?;
        
        if event::poll(std::time::Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Char('c') if key.modifiers.contains(event::KeyModifiers::CONTROL) => break,
                        _ => app.on_key(key.code),
                    }
                }
            }
        }
    }
    
    disable_raw_mode()?;
    Ok(())
}
```

---

## Future Considerations

### Multi-User Support

Future versions may support collaborative editing:

```
┌──────────┐    ┌──────────┐    ┌──────────┐
│ TUI #1   │    │ TUI #2   │    │ Web UI   │
└────┬─────┘    └────┬─────┘    └────┬─────┘
     │               │               │
     └───────────────┼───────────────┘
                     │
            ┌────────▼────────┐
            │  Brio Kernel    │
            │  (Broadcasts    │
            │   to all)       │
            └─────────────────┘
```

**Considerations**:
- Cursor position sharing
- Operational transformation for concurrent edits
- User presence indicators
- Conflict resolution UI

### Terminal Capabilities Detection

Detect and adapt to terminal features:

```rust
enum TerminalCapabilities {
    Basic,      // 16 colors, no unicode
    Standard,   // 256 colors, unicode
    TrueColor,  // 24-bit color, unicode, images
}

fn detect_capabilities() -> TerminalCapabilities {
    if supports_truecolor() && supports_unicode() {
        TerminalCapabilities::TrueColor
    } else if supports_256_colors() {
        TerminalCapabilities::Standard
    } else {
        TerminalCapabilities::Basic
    }
}
```

### Configuration Management

Support user customization via config file:

```toml
# ~/.config/brio/tui.toml
[connection]
host = "localhost"
port = 3000
reconnect_interval_ms = 5000

[ui]
theme = "dark"
show_token_usage = true
show_timestamps = false
max_history = 1000

[keybindings]
send = "ctrl+enter"
exit = "ctrl+c"
commit = "ctrl+s"
rollback = "ctrl+r"

[features]
vim_mode = false
inline_images = true
mouse_support = true
```

### Plugin Architecture

Future extensibility through plugins:

```rust
trait TuiPlugin {
    fn name(&self) -> &str;
    fn register_keybindings(&self) -> Vec<Keybinding>;
    fn render(&self, area: Rect, buf: &mut Buffer);
    fn handle_event(&self, event: Event) -> Option<Action>;
}

// Example plugins
// - Git integration panel
// - Code syntax highlighter
// - Custom color schemes
// - AI-powered autocomplete
```

---

## Summary

This guide provides the foundation for building TUI clients that integrate with Brio-Kernel. Key takeaways:

1. **WebSocket Communication**: All interaction happens through a WebSocket connection with JSON Patch-based state synchronization
2. **Headless Architecture**: The TUI is a pure client; all state lives in the kernel
3. **Real-time Updates**: JSON Patches provide efficient incremental state updates
4. **Recommended Stack**: Rust + Ratatui + Tokio + tokio-tungstenite
5. **Session Management**: Users control file changes through begin/commit/rollback workflow

For questions or contributions, refer to the [main Brio documentation](../concepts/architecture.md) or the [existing TUI development guide](../../docs/TUI_DEVELOPMENT_GUIDE.md).

---

*Last Updated: 2026-02-10*
