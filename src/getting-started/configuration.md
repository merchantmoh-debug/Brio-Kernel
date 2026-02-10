# Configuration

Brio-Kernel is configured via TOML files and environment variables. This guide covers all configuration options.

## Configuration Files

Brio looks for configuration in the following locations (in order of precedence):

1. `./brio.toml` (current directory)
2. `~/.config/brio/brio.toml` (user config)
3. `/etc/brio/brio.toml` (system config)
4. Environment variables (override all)

## Basic Configuration

Create a `brio.toml` file:

```toml
[server]
host = "127.0.0.1"
port = 8080
websocket_enabled = true

[database]
path = "./brio.db"
max_connections = 10

[logging]
level = "info"
format = "json"
file = "./brio.log"

[workspace]
base_path = "./workspace"
temp_path = "/tmp/brio"
max_file_size = "10MB"
```

## Server Configuration

```toml
[server]
# Network binding
host = "127.0.0.1"      # Bind address (use "0.0.0.0" for all interfaces)
port = 8080              # HTTP/WebSocket port

# WebSocket settings
websocket_enabled = true
websocket_path = "/ws"
heartbeat_interval = 30  # seconds
max_connections = 1000

# HTTP settings
http_enabled = true
http_path = "/api"
cors_origins = ["http://localhost:3000", "https://myapp.com"]
```

## Database Configuration

```toml
[database]
# SQLite settings
path = "./brio.db"
max_connections = 10
busy_timeout = 5000      # milliseconds

# Enable WAL mode for better concurrency
wal_enabled = true

# Migration settings
auto_migrate = true
migration_path = "./migrations"
```

## Agent Configuration

Configure default settings for all agents:

```toml
[agents]
# Default settings
max_iterations = 20
max_file_size = 1048576      # 1MB in bytes
max_depth = 10               # Directory recursion depth
timeout = 300                # seconds

# Model settings
model = "gpt-4"
temperature = 0.7
max_tokens = 4096

# Tool configuration
[agents.tools]
enable_write = true
enable_shell = false

# Shell command allowlist (only used if enable_shell = true)
[agents.tools.shell]
allowlist = ["ls", "cat", "grep", "find", "cargo", "rustc", "python", "node"]
blocklist = ["rm", "mkfs", "dd", "sudo"]
```

### Per-Agent Configuration

Override defaults for specific agents:

```toml
[agents.coder]
model = "gpt-4-turbo"
max_iterations = 30

[agents.reviewer]
model = "gpt-4"
max_iterations = 10
# Reviewer is read-only by design

[agents.council]
model = "gpt-4"
max_iterations = 5
# Council doesn't need file access

[agents.smart]
model = "gpt-4"
enable_shell = true
shell_allowlist = ["cargo", "rustc", "python", "npm"]
```

## LLM Provider Configuration

Configure one or more LLM providers:

```toml
[providers.openai]
enabled = true
api_key = "${OPENAI_API_KEY}"  # Environment variable substitution
base_url = "https://api.openai.com/v1"
default_model = "gpt-4"
timeout = 60
max_retries = 3
rate_limit = 100  # requests per minute

[providers.anthropic]
enabled = true
api_key = "${ANTHROPIC_API_KEY}"
base_url = "https://api.anthropic.com"
default_model = "claude-3-opus-20240229"
timeout = 60
max_retries = 3

[providers.local]
enabled = false
base_url = "http://localhost:11434/v1"  # Ollama example
api_key = "none"
default_model = "codellama"
```

## Security Configuration

```toml
[security]
# VFS security
max_file_size = 10485760       # 10MB
allowed_extensions = [".rs", ".py", ".js", ".ts", ".toml", ".md"]
blocked_extensions = [".exe", ".dll", ".so", ".dylib"]

# Path traversal protection
allow_absolute_paths = false
allow_symlinks = false

# SQL security
sql_policy_enabled = true
# Agents can only access tables with their prefix (e.g., coder_agent_*)

# Shell security
shell_enabled = false  # Globally disable shell commands
shell_timeout = 30     # seconds
shell_max_output = 1048576  # 1MB
```

## Logging & Telemetry

```toml
[logging]
level = "info"           # error, warn, info, debug, trace
format = "pretty"        # pretty, json, compact
file = "./brio.log"
max_size = "100MB"
max_files = 5

[telemetry]
enabled = true
exporter = "otlp"        # otlp, stdout, none
endpoint = "http://localhost:4317"
service_name = "brio-kernel"
service_version = "0.1.0"

[metrics]
enabled = true
endpoint = "127.0.0.1:9090"
path = "/metrics"
```

## Distributed Mesh Configuration

For multi-node deployments:

```toml
[mesh]
enabled = true
node_id = "node-1"
listen_addr = "0.0.0.0:50051"

# Peer nodes
[[mesh.peers]]
id = "node-2"
address = "192.168.1.2:50051"

[[mesh.peers]]
id = "node-3"
address = "192.168.1.3:50051"

# Discovery
[mesh.discovery]
method = "static"        # static, dns, kubernetes
refresh_interval = 60    # seconds
```

## Supervisor Configuration

```toml
[supervisor]
enabled = true
# Task polling interval
poll_interval = 5        # seconds

# Branching configuration
max_concurrent_branches = 5
auto_merge = false       # Require manual approval

# Merge strategy
merge_strategy = "union"  # union, ours, theirs, three-way

# Agent selection
[supervisor.agent_selection]
method = "capability"    # capability, round-robin, random
```

## Environment Variables

All configuration options can be set via environment variables using the prefix `BRIO_`:

```bash
# Server
export BRIO_SERVER_HOST="0.0.0.0"
export BRIO_SERVER_PORT=9090

# Database
export BRIO_DATABASE_PATH="/var/lib/brio/brio.db"

# Agents
export BRIO_AGENTS_MODEL="gpt-4-turbo"
export BRIO_AGENTS_MAX_ITERATIONS=30

# Providers
export OPENAI_API_KEY="sk-..."
export ANTHROPIC_API_KEY="sk-ant-..."

# Security
export BRIO_SECURITY_SHELL_ENABLED=true
```

Environment variables take precedence over configuration files.

## Configuration Examples

### Development Setup

```toml
[server]
host = "127.0.0.1"
port = 8080

[logging]
level = "debug"
format = "pretty"

[agents]
max_iterations = 10
model = "gpt-3.5-turbo"  # Cheaper for development

[agents.tools]
enable_shell = true
shell_allowlist = ["cargo", "rustc", "ls", "cat"]
```

### Production Setup

```toml
[server]
host = "0.0.0.0"
port = 443
websocket_enabled = true

[database]
path = "/var/lib/brio/brio.db"
max_connections = 50

[logging]
level = "warn"
format = "json"
file = "/var/log/brio/brio.log"

[security]
shell_enabled = false
max_file_size = 52428800  # 50MB

[agents]
model = "gpt-4"
max_iterations = 50
```

### CI/CD Setup

```toml
[server]
enabled = false  # Don't start server, just run tasks

[logging]
level = "error"
format = "compact"

[agents]
max_iterations = 5
model = "gpt-3.5-turbo"

[agents.tools]
enable_write = true
enable_shell = false
```

## Validation

Validate your configuration:

```bash
cargo run --bin brio-kernel -- --config brio.toml --check
```

This will:
- Parse the configuration file
- Check for syntax errors
- Validate environment variables
- Warn about missing required settings
- Check provider connectivity (if `--check-providers` flag is used)

## Configuration Schema

For IDE autocomplete and validation, use the JSON schema:

```bash
# Generate schema
cargo run --bin brio-kernel -- --print-schema > brio-schema.json
```

Then in VS Code with Even Better TOML extension:
```json
{
  "evenBetterToml.schema.associations": {
    "brio.toml": "file:///path/to/brio-schema.json"
  }
}
```

## Next Steps

- [Environment Variables Reference](../reference/environment-variables.md) - Complete list of environment variables
- [CLI Reference](../reference/cli-reference.md) - Command-line options
- [Troubleshooting](../reference/troubleshooting.md) - Common configuration issues
