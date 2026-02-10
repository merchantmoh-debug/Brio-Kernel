# Quick Start

Get Brio-Kernel running in under 5 minutes. This guide will walk you through your first agent execution.

## 1. Start the Kernel

```bash
cd brio-core

# Run in development mode (with logging)
RUST_LOG=info cargo run --bin brio-kernel

# Or run in production mode
cargo run --bin brio-kernel --release
```

You should see output like:

```
[2024-01-15T10:30:00Z INFO  brio_kernel] Starting Brio Kernel v0.1.0
[2024-01-15T10:30:00Z INFO  brio_kernel::host::state] BrioHostState initialized
[2024-01-15T10:30:00Z INFO  brio_kernel::ws::server] WebSocket server listening on 127.0.0.1:8080
[2024-01-15T10:30:00Z INFO  brio_kernel] Kernel ready. Press Ctrl+C to stop.
```

The kernel is now running and listening on:
- **WebSocket**: `ws://127.0.0.1:8080/ws`
- **HTTP API**: `http://127.0.0.1:8080`

## 2. Connect a Client

### Option A: Using websocat (Command Line)

```bash
# Install websocat
cargo install websocat

# Connect to kernel
websocat ws://127.0.0.1:8080/ws
```

### Option B: Using Python

```python
import asyncio
import websockets
import json

async def connect():
    uri = "ws://127.0.0.1:8080/ws"
    async with websockets.connect(uri) as websocket:
        # Receive welcome message
        welcome = await websocket.recv()
        print(f"Connected: {welcome}")
        
        # Send a task
        task = {
            "type": "create_task",
            "content": "Write a hello world program in Rust",
            "agent": "coder"
        }
        await websocket.send(json.dumps(task))
        
        # Listen for updates
        while True:
            message = await websocket.recv()
            data = json.loads(message)
            print(f"Update: {data}")

asyncio.run(connect())
```

### Option C: Using curl (HTTP API)

```bash
# Check kernel status
curl http://127.0.0.1:8080/health

# Create a task
curl -X POST http://127.0.0.1:8080/api/tasks \
  -H "Content-Type: application/json" \
  -d '{
    "content": "Review this code for bugs",
    "agent": "reviewer",
    "input_files": ["src/main.rs"]
  }'
```

## 3. Your First Agent Task

Let's use the Smart Agent to write a simple program:

### Via WebSocket

```json
{
  "type": "task",
  "agent": "smart-agent",
  "task_id": "task-001",
  "content": "Create a Python script that calculates factorials",
  "input_files": [],
  "config": {
    "max_iterations": 10,
    "enable_write": true,
    "enable_shell": false
  }
}
```

### Expected Behavior

1. **Task Creation**: Kernel creates task record in SQLite
2. **Agent Dispatch**: Supervisor selects appropriate agent
3. **Execution**: Smart Agent writes code to temp directory
4. **Completion**: Agent signals done, changes committed atomically
5. **Notification**: WebSocket broadcasts completion to all clients

### Sample Output

```json
{
  "type": "task_created",
  "task_id": "task-001",
  "status": "pending"
}
```

```json
{
  "type": "task_update",
  "task_id": "task-001",
  "status": "executing",
  "agent": "smart-agent"
}
```

```json
{
  "type": "file_change",
  "task_id": "task-001",
  "path": "factorial.py",
  "operation": "create"
}
```

```json
{
  "type": "task_completed",
  "task_id": "task-001",
  "status": "completed",
  "summary": "Created factorial.py with a recursive factorial function"
}
```

## 4. Verify the Results

Check what the agent created:

```bash
# View created file
cat factorial.py
```

Expected output:
```python
def factorial(n):
    """Calculate factorial of n."""
    if n < 0:
        raise ValueError("Factorial not defined for negative numbers")
    if n == 0 or n == 1:
        return 1
    return n * factorial(n - 1)

if __name__ == "__main__":
    # Example usage
    for i in range(6):
        print(f"{i}! = {factorial(i)}")
```

## 5. Try Different Agents

### Code Review

```json
{
  "type": "task",
  "agent": "reviewer",
  "task_id": "task-002",
  "content": "Review factorial.py for bugs and improvements",
  "input_files": ["factorial.py"]
}
```

The Reviewer Agent will:
- Read the file
- Analyze for correctness, security, performance
- Provide feedback via `<done>` tool
- **Not modify** the file (read-only for safety)

### Strategic Planning

```json
{
  "type": "task",
  "agent": "council",
  "task_id": "task-003",
  "content": "Plan the architecture for a web server that uses this factorial function",
  "input_files": ["factorial.py"]
}
```

The Council Agent will:
- Analyze requirements
- Break down into milestones
- Define dependencies
- Create strategic plan (no file operations)

## 6. Monitor with Web UI

Open your browser to `http://127.0.0.1:8080` to see:
- Active tasks
- Agent status
- System metrics
- Recent activity

## 7. Clean Up

Stop the kernel with `Ctrl+C`. All tasks and state are persisted in SQLite for resumability.

## What's Next?

- **[Configuration](configuration.md)** - Customize agents, models, and security settings
- **[Architecture](../concepts/architecture.md)** - Understand how Brio works under the hood
- **[Creating Agents](../guides/creating-agents.md)** - Build your own specialized agents
- **[TUI Integration](../guides/tui-integration.md)** - Build a terminal interface

## Common Questions

**Q: Where are files stored?**
A: By default, in `./workspace/`. The VFS creates isolated sessions in `/tmp/brio/sess-{uuid}/`.

**Q: Can I use my own LLM?**
A: Yes! Configure providers in `brio.toml`. See [Configuration](configuration.md).

**Q: How do I debug agent behavior?**
A: Enable debug logging: `RUST_LOG=debug cargo run --bin brio-kernel`

**Q: Can agents run shell commands?**
A: Only the Smart Agent with `enable_shell: true`. Commands are validated against an allowlist.

**Q: Is this safe to run on production code?**
A: Yes, with caveats. All changes go to temp directories first. Review before committing. The Reviewer Agent is read-only and safe.

---

**Congratulations!** You've successfully run your first Brio workflow. 🎉
