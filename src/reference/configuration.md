# Configuration Reference

Complete reference for all Brio-Kernel configuration options. This document covers environment variables, TOML configuration files, and their interactions.

## Table of Contents

- [Configuration Overview](#configuration-overview)
- [Database Configuration](#database-configuration)
- [Server Configuration](#server-configuration)
- [Agent Configuration](#agent-configuration)
- [Inference Configuration](#inference-configuration)
- [Telemetry Configuration](#telemetry-configuration)
- [VFS/Sandbox Configuration](#vfssandbox-configuration)
- [Distributed Mode Configuration](#distributed-mode-configuration)
- [Complete Configuration Example](#complete-configuration-example)
- [Environment Variable Reference](#environment-variable-reference)

---

## Configuration Overview

### Configuration Sources

Brio-Kernel uses a multi-layered configuration system that combines:

1. **TOML Configuration Files** - Structured configuration in files
2. **Environment Variables** - Runtime overrides with `BRIO_` prefix
3. **Default Values** - Sensible defaults built into the kernel

### Configuration Priority Order

Settings are applied in the following priority (highest to lowest):

1. **Environment Variables** (`BRIO_*`) - Override all other sources
2. **Current Directory** (`./brio.toml`) - Project-specific settings
3. **User Configuration** (`~/.config/brio/brio.toml`) - Personal defaults
4. **System Configuration** (`/etc/brio/brio.toml`) - System-wide defaults
5. **Built-in Defaults** - Kernel-provided fallback values

### Configuration File Location

```
./brio.toml                          # Project-level (recommended)
~/.config/brio/brio.toml             # User-level
/etc/brio/brio.toml                  # System-level
```

To use a custom location:

```bash
brio-kernel --config /path/to/config.toml
```

### Environment Variable Format

Environment variables use double underscore (`__`) as a separator for nested configuration:

```bash
# Maps to [server] host = "0.0.0.0"
export BRIO_SERVER__HOST="0.0.0.0"

# Maps to [server] port = 9090
export BRIO_SERVER__PORT=9090

# Maps to [telemetry] sampling_ratio = 0.5
export BRIO_TELEMETRY__SAMPLING_RATIO=0.5
```

---

## Database Configuration

### Primary Database Setting

| Variable | Default | Description | Required |
|----------|---------|-------------|----------|
| `DATABASE_URL` | `"brio.db"` | SQLite connection string | Yes |

### Connection String Formats

```
# Relative path (current directory)
brio.db

# Absolute path
/var/lib/brio/brio.db
/home/user/.local/share/brio/data.db

# In-memory (testing only)
:memory:

# With connection options
brio.db?mode=rwc&cache=shared
```

### Connection Pool Settings

When using the connection pool (via TOML configuration):

| Variable | Default | Description |
|----------|---------|-------------|
| `BRIO_DATABASE__MAX_CONNECTIONS` | `10` | Connection pool size |
| `BRIO_DATABASE__BUSY_TIMEOUT` | `5000` | Lock wait timeout (milliseconds) |
| `BRIO_DATABASE__WAL_ENABLED` | `true` | Enable Write-Ahead Logging |

### Database Configuration Examples

**Environment Variables:**

```bash
# Basic SQLite database
export DATABASE_URL="./brio.db"

# Production database location
export DATABASE_URL="/var/lib/brio/brio.db"

# Custom connection pool
export BRIO_DATABASE__MAX_CONNECTIONS=20
export BRIO_DATABASE__BUSY_TIMEOUT=10000
```

**TOML Configuration:**

```toml
[database]
url = "./brio.db"
max_connections = 10
busy_timeout = 5000
wal_enabled = true

# Migration settings
auto_migrate = true
migration_path = "./migrations"
```

---

## Server Configuration

### Server Settings

| Variable | Default | Description | Required |
|----------|---------|-------------|----------|
| `BRIO_SERVER__HOST` | `"127.0.0.1"` | Bind address | No |
| `BRIO_SERVER__PORT` | `9090` | HTTP/WebSocket port | No |

### Bind Address Options

```
127.0.0.1    # Localhost only (secure, development)
0.0.0.0      # All interfaces (production with reverse proxy)
192.168.1.10 # Specific interface
::1          # IPv6 localhost
```

### WebSocket Settings

WebSocket support is configured through TOML:

```toml
[server]
host = "127.0.0.1"
port = 9090

# WebSocket configuration
websocket_enabled = true
websocket_path = "/ws"
heartbeat_interval = 30
max_connections = 1000

# HTTP API settings
http_enabled = true
http_path = "/api"
cors_origins = ["http://localhost:3000"]
```

### Server Configuration Examples

**Development:**

```bash
export BRIO_SERVER__HOST="127.0.0.1"
export BRIO_SERVER__PORT=8080
```

**Production:**

```bash
export BRIO_SERVER__HOST="0.0.0.0"
export BRIO_SERVER__PORT=443
```

**TOML Configuration:**

```toml
[server]
host = "127.0.0.1"
port = 8080

# WebSocket settings
websocket_enabled = true
websocket_path = "/ws"
heartbeat_interval = 30
max_connections = 100
```

---

## Agent Configuration

### Global Agent Settings

All agent configuration variables use the `BRIO_AGENT_` prefix:

| Variable | Default | Description |
|----------|---------|-------------|
| `BRIO_AGENT_MAX_ITERATIONS` | `20` | Maximum ReAct loop iterations |
| `BRIO_AGENT_MODEL` | `"best-available"` | Default LLM model |
| `BRIO_AGENT_TIMEOUT_SECONDS` | `300` | Execution timeout (5 minutes) |
| `BRIO_AGENT_VERBOSE` | `false` | Enable verbose logging |
| `BRIO_AGENT_MAX_FILE_SIZE` | `10485760` | Max file size to read (10MB) |
| `BRIO_AGENT_MAX_DEPTH` | `10` | Max directory traversal depth |

### Tool Configuration

| Variable | Default | Description |
|----------|---------|-------------|
| `BRIO_AGENT_ENABLE_WRITE` | `true` | Enable file write operations |
| `BRIO_AGENT_ENABLE_SHELL` | `true` | Enable shell command execution |
| `BRIO_AGENT_SHELL_ALLOWLIST` | (see below) | Allowed shell commands |

### Default Shell Allowlist

When shell is enabled, these commands are allowed by default:

- `ls` - List directory contents
- `cat` - Display file contents
- `echo` - Print text
- `pwd` - Print working directory
- `find` - Search for files
- `grep` - Search text patterns
- `head` - Output first lines
- `tail` - Output last lines
- `wc` - Word count
- `sort` - Sort lines
- `uniq` - Report repeated lines

### Per-Agent Configuration

Override settings for specific agents in TOML:

```toml
[agents]
# Global defaults
max_iterations = 20
model = "gpt-4"
max_file_size = 1048576
max_depth = 10
timeout = 300

# Tool configuration
[agents.tools]
enable_write = true
enable_shell = true
enable_list = true

# Shell allowlist
shell_allowlist = ["ls", "cat", "grep", "find", "cargo", "rustc"]

# Agent-specific overrides
[agents.coder]
model = "gpt-4-turbo"
max_iterations = 30

[agents.reviewer]
model = "gpt-4"
max_iterations = 10

[agents.smart]
model = "gpt-4"
tool_config = { enable_shell = true }
```

### Agent Configuration Examples

**Environment Variables:**

```bash
# Development settings
export BRIO_AGENT_MAX_ITERATIONS=10
export BRIO_AGENT_MODEL="gpt-3.5-turbo"
export BRIO_AGENT_VERBOSE=true

# Production settings
export BRIO_AGENT_MAX_ITERATIONS=50
export BRIO_AGENT_MODEL="gpt-4"
export BRIO_AGENT_TIMEOUT_SECONDS=600
export BRIO_AGENT_ENABLE_SHELL=false

# Custom shell allowlist
export BRIO_AGENT_SHELL_ALLOWLIST="ls,cat,grep,cargo,rustc"
```

---

## Inference Configuration

### API Keys

| Variable | Default | Description | Required |
|----------|---------|-------------|----------|
| `OPENAI_API_KEY` | - | OpenAI API key | For OpenAI models |
| `ANTHROPIC_API_KEY` | - | Anthropic API key | For Claude models |

### API Configuration

| Variable | Default | Description |
|----------|---------|-------------|
| `OPENAI_BASE_URL` | `"https://api.openai.com/v1"` | OpenAI API base URL |

### Model Selection

Models can be specified in order of preference:

```toml
[inference]
openai_api_key = "${OPENAI_API_KEY}"
openai_base_url = "https://api.openai.com/v1"
anthropic_api_key = "${ANTHROPIC_API_KEY}"

# Model priority list
preferred_models = [
    "gpt-4-turbo",
    "gpt-4",
    "claude-3-opus-20240229",
    "claude-3-sonnet-20240229"
]

# Fallback behavior
fallback_to_available = true
```

### Inference Configuration Examples

**Environment Variables:**

```bash
# OpenAI only
export OPENAI_API_KEY="sk-..."

# With custom base URL (e.g., Azure)
export OPENAI_BASE_URL="https://your-resource.openai.azure.com/"

# Anthropic only
export ANTHROPIC_API_KEY="sk-ant-..."

# Both providers
export OPENAI_API_KEY="sk-..."
export ANTHROPIC_API_KEY="sk-ant-..."
```

**TOML Configuration:**

```toml
[inference]
openai_api_key = "${OPENAI_API_KEY}"
openai_base_url = "https://api.openai.com/v1"
anthropic_api_key = "${ANTHROPIC_API_KEY}"

# Provider priorities
[providers.openai]
enabled = true
default_model = "gpt-4"
timeout = 60
max_retries = 3

[providers.anthropic]
enabled = true
default_model = "claude-3-opus-20240229"
timeout = 60
```

---

## Telemetry Configuration

### Core Telemetry Settings

| Variable | Default | Description |
|----------|---------|-------------|
| `BRIO_TELEMETRY__SERVICE_NAME` | `"brio-kernel"` | Service identifier |
| `BRIO_TELEMETRY__OTLP_ENDPOINT` | - | OpenTelemetry endpoint |
| `BRIO_TELEMETRY__SAMPLING_RATIO` | `1.0` | Trace sampling (0.0-1.0) |

### OTLP Endpoint Formats

```
# gRPC (default)
http://localhost:4317

# HTTP/Protobuf
http://localhost:4318/v1/traces

# Jaeger
http://localhost:14268/api/traces

# Production OTLP
https://otlp.eu01.nr-data.net:4317
```

### Telemetry Configuration Examples

**Environment Variables:**

```bash
# Basic telemetry
export BRIO_TELEMETRY__SERVICE_NAME="brio-production"
export BRIO_TELEMETRY__OTLP_ENDPOINT="http://localhost:4317"
export BRIO_TELEMETRY__SAMPLING_RATIO=0.1

# High-throughput sampling
export BRIO_TELEMETRY__SAMPLING_RATIO=0.01
```

**TOML Configuration:**

```toml
[telemetry]
service_name = "brio-kernel"
otlp_endpoint = "http://localhost:4317"
sampling_ratio = 1.0

# Metrics configuration
[metrics]
enabled = true
endpoint = "127.0.0.1:9090"
path = "/metrics"
```

### Sampling Ratios

| Ratio | Use Case |
|-------|----------|
| `1.0` | Development, debugging (100% sampling) |
| `0.1` | Production with moderate volume |
| `0.01` | High-throughput production |
| `0.0` | Telemetry disabled |

---

## VFS/Sandbox Configuration

### Sandbox Settings

| Variable | Default | Description |
|----------|---------|-------------|
| `BRIO_SANDBOX__ALLOWED_PATHS` | `[]` | Allowed filesystem paths |

### Path Configuration

Paths can be absolute or relative:

```toml
[sandbox]
# Allow specific directories
allowed_paths = [
    "/home/user/projects",
    "/workspace",
    "./src"
]
```

### Session Directory Settings

Session directories are managed automatically but can be configured:

```toml
[sandbox]
# Base directory for sessions
session_base = "/var/lib/brio/sessions"

# Reflink support (copy-on-write)
reflink_enabled = true

# File size limits
max_file_size = 104857600  # 100MB
```

### Security Settings

Additional security controls:

```toml
[security]
# VFS security
max_file_size = 10485760       # 10MB
allowed_extensions = [".rs", ".py", ".js", ".ts", ".toml", ".md"]
blocked_extensions = [".exe", ".dll", ".so", ".dylib"]

# Path traversal protection
allow_absolute_paths = false
allow_symlinks = false
```

---

## Distributed Mode Configuration

### Mesh Settings

| Variable | Default | Description |
|----------|---------|-------------|
| `BRIO_MESH__ENABLED` | `false` | Enable distributed mesh |
| `BRIO_MESH__NODE_ID` | - | Unique node identifier |
| `BRIO_MESH__PORT` | - | Mesh communication port |

### Node Identification

Each node requires a unique identifier:

```bash
# Node 1
export BRIO_MESH__ENABLED=true
export BRIO_MESH__NODE_ID="node-1"
export BRIO_MESH__PORT=50051

# Node 2
export BRIO_MESH__ENABLED=true
export BRIO_MESH__NODE_ID="node-2"
export BRIO_MESH__PORT=50051
```

### Discovery Settings

```toml
[mesh]
enabled = true
node_id = "node-1"
port = 50051

# Peer nodes
[[mesh.peers]]
id = "node-2"
address = "192.168.1.2:50051"

[[mesh.peers]]
id = "node-3"
address = "192.168.1.3:50051"

# Discovery configuration
[mesh.discovery]
method = "static"        # static, dns, kubernetes
refresh_interval = 60    # seconds
```

### Branching Configuration

Distributed branching settings:

```toml
[branching]
max_concurrent_branches = 8
default_merge_strategy = "union"
allow_nested_branches = true
branch_timeout_secs = 300
line_level_diffs = true
max_nesting_depth = 3

[branching.merge_settings]
auto_merge = false
require_approval = true
```

---

## Complete Configuration Example

A production-ready `brio.toml` with all sections:

```toml
# Brio-Kernel Configuration
# Complete example for production deployment

[server]
host = "0.0.0.0"
port = 443
websocket_enabled = true
websocket_path = "/ws"
heartbeat_interval = 30
max_connections = 1000
http_enabled = true
http_path = "/api"
cors_origins = ["https://myapp.com", "https://admin.myapp.com"]

[database]
url = "/var/lib/brio/brio.db"
max_connections = 50
busy_timeout = 10000
wal_enabled = true
auto_migrate = true

[agents]
max_iterations = 50
model = "gpt-4"
max_file_size = 10485760
max_depth = 15
timeout = 600

[agents.tools]
enable_write = true
enable_shell = false
enable_list = true

[inference]
openai_api_key = "${OPENAI_API_KEY}"
openai_base_url = "https://api.openai.com/v1"
anthropic_api_key = "${ANTHROPIC_API_KEY}"

[telemetry]
service_name = "brio-production"
otlp_endpoint = "http://otel-collector:4317"
sampling_ratio = 0.1

[metrics]
enabled = true
endpoint = "0.0.0.0:9090"
path = "/metrics"

[sandbox]
allowed_paths = ["/workspace", "/var/lib/brio/data"]
session_base = "/var/lib/brio/sessions"
reflink_enabled = true

[security]
max_file_size = 104857600
allowed_extensions = [".rs", ".py", ".js", ".ts", ".toml", ".md", ".json", ".yaml"]
blocked_extensions = [".exe", ".dll", ".so", ".dylib", ".bin"]
allow_absolute_paths = false
allow_symlinks = false

[mesh]
enabled = false
node_id = "node-1"
port = 50051

[branching]
max_concurrent_branches = 8
default_merge_strategy = "union"
allow_nested_branches = true
branch_timeout_secs = 300
line_level_diffs = true
max_nesting_depth = 3

[branching.merge_settings]
auto_merge = false
require_approval = true

[logging]
level = "info"
format = "json"
file = "/var/log/brio/brio.log"
max_size = "100MB"
max_files = 5
```

---

## Environment Variable Reference

### Quick Reference Table

| Variable | Default | Description | Required |
|----------|---------|-------------|----------|
| **Database** ||||
| `DATABASE_URL` | `"brio.db"` | SQLite connection string | Yes |
| `BRIO_DATABASE__MAX_CONNECTIONS` | `10` | Connection pool size | No |
| `BRIO_DATABASE__BUSY_TIMEOUT` | `5000` | Lock timeout (ms) | No |
| `BRIO_DATABASE__WAL_ENABLED` | `true` | Enable WAL mode | No |
| **Server** ||||
| `BRIO_SERVER__HOST` | `"127.0.0.1"` | Bind address | No |
| `BRIO_SERVER__PORT` | `9090` | HTTP/WebSocket port | No |
| **Agent** ||||
| `BRIO_AGENT_MAX_ITERATIONS` | `20` | Max ReAct iterations | No |
| `BRIO_AGENT_MODEL` | `"best-available"` | Default LLM model | No |
| `BRIO_AGENT_TIMEOUT_SECONDS` | `300` | Execution timeout | No |
| `BRIO_AGENT_VERBOSE` | `false` | Verbose logging | No |
| `BRIO_AGENT_MAX_FILE_SIZE` | `10485760` | Max file size (bytes) | No |
| `BRIO_AGENT_MAX_DEPTH` | `10` | Max directory depth | No |
| `BRIO_AGENT_ENABLE_WRITE` | `true` | Enable file writes | No |
| `BRIO_AGENT_ENABLE_SHELL` | `true` | Enable shell commands | No |
| `BRIO_AGENT_SHELL_ALLOWLIST` | (see docs) | Allowed commands | No |
| **Inference** ||||
| `OPENAI_API_KEY` | - | OpenAI API key | Conditionally |
| `OPENAI_BASE_URL` | `"https://api.openai.com/v1"` | OpenAI base URL | No |
| `ANTHROPIC_API_KEY` | - | Anthropic API key | Conditionally |
| **Telemetry** ||||
| `BRIO_TELEMETRY__SERVICE_NAME` | `"brio-kernel"` | Service identifier | No |
| `BRIO_TELEMETRY__OTLP_ENDPOINT` | - | OTLP endpoint | No |
| `BRIO_TELEMETRY__SAMPLING_RATIO` | `1.0` | Sampling ratio | No |
| **Sandbox** ||||
| `BRIO_SANDBOX__ALLOWED_PATHS` | `[]` | Allowed filesystem paths | No |
| **Mesh** ||||
| `BRIO_MESH__ENABLED` | `false` | Enable distributed mesh | No |
| `BRIO_MESH__NODE_ID` | - | Unique node ID | No |
| `BRIO_MESH__PORT` | - | Mesh port | No |

### Type Conventions

| Type | Format | Example |
|------|--------|---------|
| String | Plain text | `"value"` |
| Integer | Numeric | `9090` |
| Float | Decimal | `0.5` |
| Boolean | `true`/`false` | `true` |
| Duration | Seconds (int) | `300` |
| List | Comma-separated | `"a,b,c"` |

### Validation

Validate your configuration before starting:

```bash
# Check environment variables
brio-kernel --check-config

# Print effective configuration
brio-kernel --print-config

# Test with dry-run
brio-kernel --dry-run
```

### Security Notes

**Sensitive Variables:**

- `OPENAI_API_KEY`
- `ANTHROPIC_API_KEY`
- `DATABASE_URL` (if contains credentials)

**Best Practices:**

1. Never commit API keys to version control
2. Use `.env` files (add to `.gitignore`)
3. Use secret management tools (Vault, AWS Secrets Manager)
4. Rotate keys regularly
5. Prevent shell history storage:

```bash
# Add space before command (Bash)
 export OPENAI_API_KEY="sk-..."

# Configure Zsh
setopt HIST_IGNORE_SPACE
```

---

## See Also

- [Getting Started: Configuration](../getting-started/configuration.md) - Basic setup guide
- [Environment Variables](./environment-variables.md) - Detailed environment variable guide
- [Troubleshooting](./troubleshooting.md) - Common configuration issues
- [CLI Reference](./cli-reference.md) - Command-line options
