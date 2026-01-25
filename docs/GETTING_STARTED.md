# Getting Started with Brio Kernel

This guide walks you through setting up, building, and running the Brio Kernel.

---

## Prerequisites

| Requirement | Version | Purpose            |
| ----------- | ------- | ------------------ |
| **Rust**    | 1.75+   | Kernel compilation |
| **SQLite**  | 3.x     | State database     |
| **Git**     | 2.x     | Source control     |

### Optional

| Requirement                 | Purpose                 |
| --------------------------- | ----------------------- |
| **OpenTelemetry Collector** | Observability export    |
| **wasm-tools**              | WASM component building |

---

## Installation

### 1. Clone the Repository

```bash
git clone https://github.com/Wilbatronic/Brio-Kernel.git
cd Brio-Kernel/brio-core
```

### 2. Build the Kernel

```bash
# Debug build
cargo build

# Release build (optimized)
cargo build --release
```

### 3. Run Tests

```bash
# All tests
cargo test

# With output
cargo test -- --nocapture

# Specific test
cargo test vfs_tests
```

---

## Configuration

### Environment Variables

| Variable         | Default            | Description                      |
| ---------------- | ------------------ | -------------------------------- |
| `DATABASE_URL`   | `sqlite://brio.db` | SQLite connection string         |
| `SERVER_HOST`    | `127.0.0.1`        | Server bind address              |
| `SERVER_PORT`    | `3000`             | Server port                      |
| `OTLP_ENDPOINT`  | (none)             | OpenTelemetry collector endpoint |
| `SAMPLING_RATIO` | `1.0`              | Trace sampling ratio (0.0-1.0)   |

### Configuration File

Create `config.toml` in the kernel directory:

```toml
[database]
url = "sqlite://brio.db"

[server]
host = "127.0.0.1"
port = 3000

[telemetry]
otlp_endpoint = "http://localhost:4317"
sampling_ratio = 1.0
```

---

## Running the Kernel

### Development Mode

```bash
cargo run
```

Expected output:
```
INFO brio_kernel: Brio Kernel Starting...
INFO brio_kernel: Brio Kernel Initialized. Waiting for shutdown signal...
```

### Production Mode

```bash
cargo run --release
```

### With Custom Config

```bash
DATABASE_URL="sqlite://custom.db" cargo run
```

---

## Connecting Clients

### WebSocket Endpoint

```
ws://localhost:3000/ws
```

### Testing with websocat

```bash
# Install websocat
cargo install websocat

# Connect
websocat ws://localhost:3000/ws

# You'll receive JSON Patch messages as state changes
```

### Example Python Client

```python
import asyncio
import websockets
import json

async def connect():
    async with websockets.connect("ws://localhost:3000/ws") as ws:
        # Send a task
        await ws.send(json.dumps({
            "type": "task",
            "content": "Hello, Brio!"
        }))
        
        # Receive updates
        async for message in ws:
            patch = json.loads(message)
            print(f"Received: {patch}")

asyncio.run(connect())
```

---

## Database Setup

Brio uses SQLite for state management. The database is created automatically on first run.

### Manual Schema Setup

```bash
sqlite3 brio.db < schema.sql
```

### Inspecting the Database

```bash
sqlite3 brio.db
.tables
.schema tasks
SELECT * FROM tasks;
```

---

## Building WASM Components

### Prerequisites

```bash
# Install wasm-tools
cargo install wasm-tools

# Add WASM target
rustup target add wasm32-wasi
```

### Build a Component

```bash
cd components/tools
cargo build --target wasm32-wasi --release

# Convert module to component
wasm-tools component new \
  target/wasm32-wasi/release/tool_grep.wasm \
  -o tool_grep.component.wasm
```

---

## Project Structure

```
Brio-Kernel/
├── README.md               # Project overview
├── LICENSE                 # License (GPL-3.0)
├── CONTRIBUTING.md         # Contribution guidelines
├── docs/                   # Documentation
│   ├── ARCHITECTURE.md     # System architecture
│   ├── GETTING_STARTED.md  # This file
│   ├── API_REFERENCE.md    # API documentation
│   └── TUI_DEVELOPMENT_GUIDE.md
└── brio-core/              # Main source code
    ├── Cargo.toml
    ├── kernel/             # Rust kernel
    ├── components/         # WASM components
    └── wit/                # Interface definitions
```

---

## Common Tasks

### Adding a New Tool

1. Create component in `components/tools/`
2. Implement WIT interface from `wit/tools.wit`
3. Build as WASM component
4. Register in linker

### Modifying WIT Interfaces

1. Edit files in `wit/`
2. Regenerate bindings: `cargo build` (auto-regenerates)
3. Update component implementations

### Adding New Inference Provider

1. Implement `LLMProvider` trait
2. Add to `inference/mod.rs` exports
3. Configure in `main.rs`

---

## Troubleshooting

### Build Errors

**Problem:** `cannot find -lsqlite3`
```bash
# Ubuntu/Debian
sudo apt install libsqlite3-dev

# macOS
brew install sqlite

# Windows
# Use vcpkg or download SQLite dll
```

**Problem:** WASM target not found
```bash
rustup target add wasm32-wasi
```

### Runtime Errors

**Problem:** "Failed to connect to database"
- Check `DATABASE_URL` environment variable
- Ensure SQLite is installed
- Verify file permissions

**Problem:** "Address already in use"
- Another process using port 3000
- Change `SERVER_PORT` or stop other process

### WebSocket Connection Issues

**Problem:** Connection refused
- Verify kernel is running
- Check firewall settings
- Confirm correct port

---

## Next Steps

1. **Read the Architecture Guide** → [ARCHITECTURE.md](./ARCHITECTURE.md)
2. **Explore the API** → [API_REFERENCE.md](./API_REFERENCE.md)
3. **Build a TUI** → [TUI_DEVELOPMENT_GUIDE.md](./TUI_DEVELOPMENT_GUIDE.md)
4. **Contribute** → [CONTRIBUTING.md](../CONTRIBUTING.md)

---

## Support

- **Issues**: [GitHub Issues](https://github.com/Wilbatronic/Brio-Kernel/issues)
- **Discussions**: [GitHub Discussions](https://github.com/Wilbatronic/Brio-Kernel/discussions)
