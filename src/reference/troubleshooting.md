# Troubleshooting

Common issues and solutions for Brio-Kernel.

## Installation Issues

### WASM Target Not Found

**Error:**
```
error: target `wasm32-wasi` not found
```

**Solution:**
```bash
rustup target add wasm32-wasi
```

### wasm-tools Not Found

**Error:**
```
wasm-tools: command not found
```

**Solution:**
```bash
cargo install wasm-tools
```

### Build Failures

**Error:**
```
linking with `cc` failed
```

**Solution:**
```bash
# Ubuntu/Debian
sudo apt-get update
sudo apt-get install -y build-essential pkg-config libssl-dev

# macOS
xcode-select --install

# Then clean and rebuild
cargo clean
cargo build --workspace
```

## Runtime Issues

### Kernel Won't Start

**Symptoms:** Kernel exits immediately or crashes on startup

**Solutions:**

1. **Check port availability:**
```bash
lsof -i :8080  # Check if port is in use
```

2. **Verify database:**
```bash
ls -la brio.db  # Check database exists
sqlite3 brio.db ".tables"  # Verify schema
```

3. **Check permissions:**
```bash
# Ensure write permissions
chmod +x ./brio-core
```

4. **Check logs:**
```bash
RUST_LOG=debug cargo run --bin brio-kernel
```

### WebSocket Connection Failed

**Error:**
```
WebSocket connection to 'ws://localhost:8080/ws' failed
```

**Solutions:**

1. **Verify kernel is running:**
```bash
curl http://localhost:8080/health
```

2. **Check firewall:**
```bash
# Temporarily disable firewall for testing
sudo ufw disable  # Ubuntu
```

3. **Check WebSocket path:**
```bash
# Verify correct path
curl -i -N \
  -H "Connection: Upgrade" \
  -H "Upgrade: websocket" \
  -H "Host: localhost:8080" \
  -H "Origin: http://localhost:8080" \
  http://localhost:8080/ws
```

### Task Not Being Processed

**Symptoms:** Task stays in "pending" status

**Solutions:**

1. **Check supervisor:**
```bash
# Verify supervisor is enabled
grep supervisor.enabled brio.toml
```

2. **Check agent availability:**
```bash
# List available agents
curl http://localhost:8080/api/agents
```

3. **Check database:**
```bash
sqlite3 brio.db "SELECT * FROM tasks WHERE status='pending';"
```

4. **Check logs:**
```bash
tail -f brio.log | grep supervisor
```

## Agent Issues

### Agent Fails Immediately

**Symptoms:** Agent returns error on first iteration

**Solutions:**

1. **Check model availability:**
```bash
# Verify API key
echo $OPENAI_API_KEY

# Test API
curl https://api.openai.com/v1/models \
  -H "Authorization: Bearer $OPENAI_API_KEY"
```

2. **Check agent configuration:**
```bash
# Verify agent is registered
curl http://localhost:8080/api/agents/coder
```

3. **Check WASM component:**
```bash
ls -la brio-core/target/wasm32-wasi/debug/coder_agent.wasm
```

### Agent Times Out

**Symptoms:** Task exceeds timeout limit

**Solutions:**

1. **Increase timeout:**
```toml
[agents]
timeout = 600  # 10 minutes
```

2. **Increase max iterations:**
```toml
[agents]
max_iterations = 50
```

3. **Simplify task:**
Break complex tasks into smaller chunks

### Shell Command Rejected

**Error:**
```
Shell command rejected: not in allowlist
```

**Solution:**
```toml
[agents.smart]
enable_shell = true
shell_allowlist = ["cargo", "rustc", "ls", "cat", "grep"]
```

## Database Issues

### Database Locked

**Error:**
```
database is locked
```

**Solutions:**

1. **Enable WAL mode:**
```toml
[database]
wal_enabled = true
```

2. **Increase busy timeout:**
```toml
[database]
busy_timeout = 10000  # 10 seconds
```

3. **Check for long-running transactions:**
```bash
sqlite3 brio.db ".tables"
```

### Schema Errors

**Error:**
```
table does not exist
```

**Solution:**
```bash
# Rebuild database
rm brio.db
cargo run --bin brio-kernel
```

## Performance Issues

### High Memory Usage

**Symptoms:** Kernel uses excessive memory

**Solutions:**

1. **Limit concurrent agents:**
```toml
[supervisor]
max_concurrent_branches = 3
```

2. **Reduce file size limits:**
```toml
[agents]
max_file_size = 524288  # 512KB
```

3. **Enable release mode:**
```bash
cargo run --bin brio-kernel --release
```

### Slow Response Times

**Symptoms:** High latency for WebSocket responses

**Solutions:**

1. **Check LLM latency:**
```bash
# Test LLM response time
curl -w "@curl-format.txt" \
  https://api.openai.com/v1/chat/completions
```

2. **Use faster model:**
```toml
[agents]
model = "gpt-3.5-turbo"
```

3. **Enable caching:**
```toml
[agents]
cache_responses = true
```

## WASM Component Issues

### Component Won't Load

**Error:**
```
Failed to instantiate component
```

**Solutions:**

1. **Rebuild component:**
```bash
cargo build -p coder-agent --target wasm32-wasi
```

2. **Check WIT files:**
```bash
# Verify WIT is valid
wasm-tools component wit brio-core/wit/
```

3. **Check component version:**
```bash
wasm-tools print target/wasm32-wasi/debug/coder_agent.wasm
```

### WIT Interface Mismatch

**Error:**
```
WIT interface mismatch
```

**Solution:**
```bash
# Regenerate bindings
cd brio-core/components/coder
wit-bindgen rust --out-dir src/ ../../../wit/
```

## Security Issues

### Path Traversal Attempt

**Log:**
```
Path validation failed: path traversal detected
```

**Solution:**
This is expected behavior. Agents are blocked from accessing files outside their VFS session.

### Dangerous Command Blocked

**Log:**
```
Shell command blocked: dangerous command detected
```

**Solution:**
This is expected security behavior. Review agent prompts if this happens unexpectedly.

## Getting Help

If you're still stuck:

1. **Enable debug logging:**
```bash
RUST_LOG=debug cargo run --bin brio-kernel 2>&1 | tee debug.log
```

2. **Check system requirements:**
```bash
# Verify versions
rustc --version  # Should be 1.93+
sqlite3 --version  # Should be 3.x
wasm-tools --version
```

3. **Run diagnostics:**
```bash
# Health check
curl http://localhost:8080/health

# Metrics
curl http://localhost:8080/metrics
```

4. **Report issues:**
- Include debug logs
- Describe steps to reproduce
- Include system information

## Common Error Codes

| Code | Meaning | Solution |
|------|---------|----------|
| E001 | Database connection failed | Check SQLite installation |
| E002 | WASM component load failed | Rebuild components |
| E003 | LLM API error | Check API key and connectivity |
| E004 | VFS session error | Check disk space and permissions |
| E005 | Tool execution failed | Check tool configuration |
| E006 | Task timeout | Increase timeout or simplify task |
| E007 | Security violation | Review agent configuration |
