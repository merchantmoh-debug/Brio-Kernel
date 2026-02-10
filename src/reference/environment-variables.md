# Environment Variables

Complete reference of all environment variables supported by Brio-Kernel.

## Prefix Convention

All Brio environment variables use the prefix `BRIO_`.

```bash
# General format
BRIO_<COMPONENT>_<SETTING>
```

## Core Settings

### Server

| Variable | Description | Default |
|----------|-------------|---------|
| `BRIO_SERVER_HOST` | Bind address | `127.0.0.1` |
| `BRIO_SERVER_PORT` | HTTP/WebSocket port | `8080` |
| `BRIO_SERVER_WEBSOCKET_ENABLED` | Enable WebSocket | `true` |
| `BRIO_SERVER_HTTP_ENABLED` | Enable HTTP API | `true` |
| `BRIO_SERVER_MAX_CONNECTIONS` | Max concurrent connections | `1000` |

### Database

| Variable | Description | Default |
|----------|-------------|---------|
| `BRIO_DATABASE_PATH` | SQLite database path | `./brio.db` |
| `BRIO_DATABASE_MAX_CONNECTIONS` | Connection pool size | `10` |
| `BRIO_DATABASE_WAL_ENABLED` | Enable WAL mode | `true` |
| `BRIO_DATABASE_BUSY_TIMEOUT` | Lock wait timeout (ms) | `5000` |

### Logging

| Variable | Description | Default |
|----------|-------------|---------|
| `BRIO_LOGGING_LEVEL` | Log level | `info` |
| `BRIO_LOGGING_FORMAT` | Log format | `pretty` |
| `BRIO_LOGGING_FILE` | Log file path | `./brio.log` |

### Workspace

| Variable | Description | Default |
|----------|-------------|---------|
| `BRIO_WORKSPACE_BASE_PATH` | Working directory | `./workspace` |
| `BRIO_WORKSPACE_TEMP_PATH` | Temp directory | `/tmp/brio` |
| `BRIO_WORKSPACE_MAX_FILE_SIZE` | Max file size | `10485760` |

## Agent Settings

### Global Agent Configuration

| Variable | Description | Default |
|----------|-------------|---------|
| `BRIO_AGENTS_MAX_ITERATIONS` | Max ReAct iterations | `20` |
| `BRIO_AGENTS_MAX_FILE_SIZE` | Max file read size (bytes) | `1048576` |
| `BRIO_AGENTS_MAX_DEPTH` | Max directory depth | `10` |
| `BRIO_AGENTS_TIMEOUT` | Task timeout (seconds) | `300` |
| `BRIO_AGENTS_MODEL` | Default LLM model | `gpt-4` |
| `BRIO_AGENTS_TEMPERATURE` | LLM temperature | `0.7` |
| `BRIO_AGENTS_MAX_TOKENS` | Max LLM response tokens | `4096` |

### Tool Configuration

| Variable | Description | Default |
|----------|-------------|---------|
| `BRIO_AGENTS_TOOLS_ENABLE_WRITE` | Enable file writes | `true` |
| `BRIO_AGENTS_TOOLS_ENABLE_SHELL` | Enable shell commands | `false` |

### Per-Agent Settings

| Variable | Description | Applies To |
|----------|-------------|------------|
| `BRIO_AGENTS_CODER_MODEL` | Coder agent model | Coder |
| `BRIO_AGENTS_CODER_MAX_ITERATIONS` | Coder max iterations | Coder |
| `BRIO_AGENTS_REVIEWER_MODEL` | Reviewer agent model | Reviewer |
| `BRIO_AGENTS_SMART_MODEL` | Smart agent model | Smart |
| `BRIO_AGENTS_SMART_ENABLE_SHELL` | Smart agent shell access | Smart |

## Provider Settings

### OpenAI

| Variable | Description | Required |
|----------|-------------|----------|
| `OPENAI_API_KEY` | OpenAI API key | Yes |
| `OPENAI_BASE_URL` | API base URL | No |
| `OPENAI_DEFAULT_MODEL` | Default model | No |

### Anthropic

| Variable | Description | Required |
|----------|-------------|----------|
| `ANTHROPIC_API_KEY` | Anthropic API key | Yes |
| `ANTHROPIC_BASE_URL` | API base URL | No |

### Local Models

| Variable | Description | Default |
|----------|-------------|---------|
| `BRIO_PROVIDERS_LOCAL_BASE_URL` | Local API URL | `http://localhost:11434` |
| `BRIO_PROVIDERS_LOCAL_API_KEY` | Local API key | `none` |

## Security Settings

| Variable | Description | Default |
|----------|-------------|---------|
| `BRIO_SECURITY_MAX_FILE_SIZE` | Max file size (bytes) | `10485760` |
| `BRIO_SECURITY_ALLOW_ABSOLUTE_PATHS` | Allow absolute paths | `false` |
| `BRIO_SECURITY_ALLOW_SYMLINKS` | Allow symlinks | `false` |
| `BRIO_SECURITY_SHELL_ENABLED` | Global shell enable | `false` |
| `BRIO_SECURITY_SHELL_TIMEOUT` | Shell timeout (seconds) | `30` |

## Supervisor Settings

| Variable | Description | Default |
|----------|-------------|---------|
| `BRIO_SUPERVISOR_ENABLED` | Enable supervisor | `true` |
| `BRIO_SUPERVISOR_POLL_INTERVAL` | Task poll interval (seconds) | `5` |
| `BRIO_SUPERVISOR_MAX_CONCURRENT_BRANCHES` | Max parallel branches | `5` |
| `BRIO_SUPERVISOR_AUTO_MERGE` | Auto-merge branches | `false` |
| `BRIO_SUPERVISOR_MERGE_STRATEGY` | Default merge strategy | `union` |

## Mesh Settings

| Variable | Description | Default |
|----------|-------------|---------|
| `BRIO_MESH_ENABLED` | Enable distributed mesh | `false` |
| `BRIO_MESH_NODE_ID` | Unique node identifier | `node-1` |
| `BRIO_MESH_LISTEN_ADDR` | gRPC listen address | `0.0.0.0:50051` |

## Telemetry Settings

| Variable | Description | Default |
|----------|-------------|---------|
| `BRIO_TELEMETRY_ENABLED` | Enable telemetry | `true` |
| `BRIO_TELEMETRY_EXPORTER` | Telemetry exporter | `otlp` |
| `BRIO_TELEMETRY_ENDPOINT` | OTLP endpoint | `http://localhost:4317` |
| `BRIO_TELEMETRY_SERVICE_NAME` | Service name | `brio-kernel` |

## Metrics Settings

| Variable | Description | Default |
|----------|-------------|---------|
| `BRIO_METRICS_ENABLED` | Enable metrics | `true` |
| `BRIO_METRICS_ENDPOINT` | Metrics bind address | `127.0.0.1:9090` |

## Usage Examples

### Development Setup

```bash
# Server configuration
export BRIO_SERVER_HOST="127.0.0.1"
export BRIO_SERVER_PORT=8080

# Logging
export BRIO_LOGGING_LEVEL="debug"
export BRIO_LOGGING_FORMAT="pretty"

# Agents
export BRIO_AGENTS_MODEL="gpt-3.5-turbo"
export BRIO_AGENTS_MAX_ITERATIONS=10

# LLM Provider
export OPENAI_API_KEY="sk-..."
```

### Production Setup

```bash
# Server
export BRIO_SERVER_HOST="0.0.0.0"
export BRIO_SERVER_PORT=443

# Logging
export BRIO_LOGGING_LEVEL="warn"
export BRIO_LOGGING_FORMAT="json"
export BRIO_LOGGING_FILE="/var/log/brio/brio.log"

# Security
export BRIO_SECURITY_SHELL_ENABLED=false
export BRIO_AGENTS_TOOLS_ENABLE_SHELL=false

# Database
export BRIO_DATABASE_PATH="/var/lib/brio/brio.db"
```

### CI/CD Setup

```bash
# Minimal configuration
export BRIO_SERVER_ENABLED=false
export BRIO_LOGGING_LEVEL="error"
export BRIO_AGENTS_MAX_ITERATIONS=5
export OPENAI_API_KEY="${OPENAI_API_KEY}"
```

### Distributed Mesh

```bash
# Node 1
export BRIO_MESH_ENABLED=true
export BRIO_MESH_NODE_ID="node-1"
export BRIO_MESH_LISTEN_ADDR="0.0.0.0:50051"

# Node 2
export BRIO_MESH_ENABLED=true
export BRIO_MESH_NODE_ID="node-2"
export BRIO_MESH_LISTEN_ADDR="0.0.0.0:50051"
```

## Priority Order

Environment variables override configuration file settings:

1. **Environment Variables** (highest priority)
2. `./brio.toml` (current directory)
3. `~/.config/brio/brio.toml` (user config)
4. `/etc/brio/brio.toml` (system config)

## Validation

To verify environment variables are being read:

```bash
# Print effective configuration
cargo run --bin brio-kernel -- --print-config

# Check specific value
echo $BRIO_SERVER_PORT
```

## Security Notes

### Sensitive Variables

Keep these secure:

- `OPENAI_API_KEY`
- `ANTHROPIC_API_KEY`
- `BRIO_PROVIDERS_LOCAL_API_KEY`

Best practices:

1. Use `.env` files (not committed)
2. Use secret management (Vault, AWS Secrets Manager)
3. Never log API keys
4. Rotate keys regularly

### Shell History

Prevent sensitive values from being saved in shell history:

```bash
# Bash: Add space before command
 export OPENAI_API_KEY="sk-..."

# Zsh: Configure HIST_IGNORE_SPACE
setopt HIST_IGNORE_SPACE
```
