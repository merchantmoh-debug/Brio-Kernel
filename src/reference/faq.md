# Frequently Asked Questions

## General Questions

### What is Brio-Kernel?

Brio-Kernel is a strictly headless micro-kernel designed to orchestrate AI agents using the WebAssembly Component Model (WASI Preview 2). It provides a secure, high-performance platform for running autonomous AI agents that can read, write, and execute code.

### Who should use Brio?

Brio is ideal for:
- **Developers** who want AI assistance with coding
- **Teams** that need automated code review
- **Organizations** building AI-powered development tools
- **Researchers** exploring multi-agent systems

### Is Brio production-ready?

Brio is under active development. Core features are stable, but you should:
- Review all AI-generated code before committing
- Use version control
- Test thoroughly in staging environments

## Getting Started

### How do I install Brio?

See the [Installation Guide](../getting-started/installation.md) for detailed instructions. Quick start:

```bash
git clone https://github.com/Brio-Kernel/brio-kernel.git
cd brio-kernel
rustup target add wasm32-wasi
cargo build --workspace
```

### What are the system requirements?

- **Rust**: 1.93 or higher
- **OS**: Linux, macOS, Windows (WSL recommended)
- **RAM**: 4GB minimum, 8GB recommended
- **Disk**: 2GB for installation

### Do I need an LLM API key?

Yes, Brio requires an LLM provider. We support:
- OpenAI (GPT-4, GPT-3.5)
- Anthropic (Claude)
- Local models via Ollama

Set your API key:
```bash
export OPENAI_API_KEY="sk-..."
```

## Agents

### What's the difference between agents?

| Agent | Purpose | Write Access | Shell |
|-------|---------|--------------|-------|
| **Coder** | Write code | ✅ | ❌ |
| **Reviewer** | Review code | ❌ (safety) | ❌ |
| **Council** | Strategic planning | ❌ | ❌ |
| **Foreman** | Task orchestration | ❌ | ❌ |
| **Smart Agent** | General-purpose | ✅ | ✅ (configurable) |

### Which agent should I use?

- **Writing code** → Coder Agent
- **Code review** → Reviewer Agent
- **Planning/architecture** → Council Agent
- **Complex tasks with commands** → Smart Agent
- **Task automation** → Foreman Agent

### Can I create custom agents?

Yes! See the [Creating Agents](../guides/creating-agents.md) guide. You can:
- Implement the `StandardAgent` trait
- Define custom tools
- Configure your own prompts
- Build WASM components

### Why is the Reviewer Agent read-only?

**Security by design.** The Reviewer Agent cannot modify code to prevent:
- Accidental changes during review
- Malicious modifications
- Violation of separation of concerns

This makes it safe to run reviews on any codebase.

## Security

### Is Brio secure?

Yes, Brio implements multiple security layers:

1. **WASM Sandbox** - Agents run in isolated WebAssembly
2. **VFS Sessions** - File changes are isolated in temp directories
3. **Capability Model** - Each agent has specific permissions
4. **Path Validation** - Prevents path traversal attacks
5. **Shell Allowlist** - Only approved commands can run

### Can agents delete my files?

**No.** Even agents with write access cannot:
- Delete files (rm is blocked)
- Access files outside their session
- Execute dangerous commands

All changes go to a temporary directory first and must be explicitly committed.

### How do I control what agents can do?

Use configuration:

```toml
[agents.smart]
enable_write = true      # Allow file writes
enable_shell = false     # Disable shell commands
```

Or disable tools entirely:
```toml
[agents.tools]
enable_write = false
enable_shell = false
```

### Can I audit what agents do?

Yes, Brio provides:
- **Audit logs** - All actions are logged
- **WebSocket updates** - Real-time monitoring
- **Metrics** - Usage statistics
- **Structured logging** - JSON format for analysis

## Configuration

### Where is configuration stored?

Brio looks for `brio.toml` in order:
1. `./brio.toml` (current directory)
2. `~/.config/brio/brio.toml`
3. `/etc/brio/brio.toml`

### Can I use environment variables?

Yes! All settings can use `BRIO_` prefix:

```bash
export BRIO_SERVER_PORT=9090
export BRIO_AGENTS_MODEL="gpt-4-turbo"
export OPENAI_API_KEY="sk-..."
```

### How do I enable shell commands?

For Smart Agent only:

```toml
[agents.smart]
enable_shell = true
shell_allowlist = ["cargo", "rustc", "python"]
```

**Warning:** Only enable shell if you trust the agent and validate the allowlist.

### Can I use multiple LLM providers?

Yes:

```toml
[providers.openai]
api_key = "${OPENAI_API_KEY}"

[providers.anthropic]
api_key = "${ANTHROPIC_API_KEY}"
```

Agents can select their preferred provider.

## Troubleshooting

### Why is my task stuck in "pending"?

Common causes:
1. Supervisor not enabled
2. No agents available
3. Database connection issue

Check:
```bash
curl http://localhost:8080/health
curl http://localhost:8080/api/agents
```

### Why did my agent fail?

Check logs:
```bash
RUST_LOG=debug cargo run --bin brio-kernel 2>&1 | grep ERROR
```

Common issues:
- LLM API key invalid
- File too large
- Timeout exceeded
- Tool not available

### How do I debug agent behavior?

1. **Enable debug logging:**
```bash
RUST_LOG=debug cargo run --bin brio-kernel
```

2. **Monitor WebSocket:**
```bash
websocat ws://localhost:8080/ws
```

3. **Check task status:**
```bash
curl http://localhost:8080/api/tasks/task-001
```

## Performance

### How fast is Brio?

Typical performance:
- **Task dispatch**: <10ms
- **Agent startup**: <500ms
- **File operations**: <50ms
- **LLM response**: 1-30s (depends on provider)

### Can I run multiple agents in parallel?

Yes! Use branching:

```toml
[supervisor]
max_concurrent_branches = 5
```

Each branch can run a different agent or approach.

### Does Brio support distributed deployment?

Yes! Enable distributed mesh:

```toml
[mesh]
enabled = true
node_id = "node-1"
listen_addr = "0.0.0.0:50051"
```

See [Distributed Mesh](../guides/distributed-mesh.md) for details.

## Development

### How do I build custom tools?

See [Creating Tools](../guides/creating-tools.md). Tools are:
- WASM components
- Implement the `Tool` trait
- Type-safe via WIT interfaces
- Sandboxed for security

### Can I integrate Brio with my IDE?

Yes! Brio provides:
- **WebSocket API** - Real-time updates
- **HTTP API** - RESTful endpoints
- **JSON Patch** - State synchronization

See [TUI Integration](../guides/tui-integration.md) for examples.

### How do I contribute?

See [CONTRIBUTING.md](../../CONTRIBUTING.md). We welcome:
- Bug reports
- Feature requests
- Documentation improvements
- Code contributions

### Is there a Discord/Slack community?

Not yet, but you can:
- Open [GitHub Discussions](https://github.com/Brio-Kernel/brio-kernel/discussions)
- Join the mailing list (coming soon)
- Follow on Twitter (coming soon)

## Comparison

### How does Brio compare to Claude Code?

| Feature | Brio | Claude Code |
|---------|------|-------------|
| **Architecture** | Micro-kernel | Monolithic |
| **Agents** | Multiple specialized | Single |
| **Sandbox** | WASM | Process |
| **Extensibility** | High (custom agents/tools) | Limited |
| **Self-hosted** | ✅ | ❌ |

### How does Brio compare to AutoGPT?

| Feature | Brio | AutoGPT |
|---------|------|---------|
| **Safety** | High (WASM sandbox) | Medium |
| **Specialization** | Multiple agents | Single agent |
| **State Management** | SQLite + VFS | File-based |
| **Tool System** | Type-safe WIT | Python functions |
| **Production Ready** | Designed for it | Experimental |

### Why not just use the OpenAI API directly?

Brio provides:
- **Multi-agent orchestration** - Coordinate multiple AI agents
- **Security** - Sandboxed execution
- **State management** - Persistent task state
- **Tool system** - Type-safe tools
- **Extensibility** - Custom agents and tools

## Pricing

### Is Brio free?

Brio is **open source** (MPL-2.0 license). You pay for:
- Your own infrastructure
- LLM API usage (OpenAI, Anthropic, etc.)

### How much do LLM calls cost?

Depends on the provider and usage:

**OpenAI GPT-4:**
- Input: $0.03 per 1K tokens
- Output: $0.06 per 1K tokens

**OpenAI GPT-3.5:**
- Input: $0.0015 per 1K tokens
- Output: $0.002 per 1K tokens

Typical task: $0.01-0.10

### Can I use local models to save money?

Yes! Configure Ollama or similar:

```toml
[providers.local]
enabled = true
base_url = "http://localhost:11434"
model = "codellama"
```

## Future

### What's on the roadmap?

**Implemented:**
- ✅ Multi-agent support
- ✅ Branching and merging
- ✅ Distributed mesh
- ✅ WebSocket API

**Planned:**
- 🚧 Component hot-reload
- 🚧 Persistent sessions
- 🚧 Plugin marketplace
- 🚧 Web UI

### When will feature X be available?

Check [GitHub Issues](https://github.com/Brio-Kernel/brio-kernel/issues) for:
- Roadmap
- Feature requests
- Release planning

### How can I request a feature?

Open a [GitHub Issue](https://github.com/Brio-Kernel/brio-kernel/issues/new) with:
- Clear description
- Use case
- Proposed solution (optional)

## Still Have Questions?

- 📖 [Full Documentation](../SUMMARY.md)
- 🐛 [Report Issues](https://github.com/Brio-Kernel/brio-kernel/issues)
- 💬 [GitHub Discussions](https://github.com/Brio-Kernel/brio-kernel/discussions)
- 🤝 [Contributing](../../CONTRIBUTING.md)
