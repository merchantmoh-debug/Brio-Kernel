# Shell Tool API Reference

The Shell Tool is a WASM component that provides controlled shell command execution to agents with comprehensive security validation.

## Overview

Executes system commands directly without shell interpolation, preventing command injection attacks through input validation.

**Use Cases**: Directory listing, file inspection, build commands, safe system utilities  
**Avoid For**: Commands with shell operators (`|`, `;`), destructive operations, or complex scripts

## Security Model

### Blocked Patterns

```rust
const DANGEROUS_CHARS: &[char] = &[
    '|', ';', '&', '$', '`', '>', '<', '(', ')', '{', '}'
];

const DANGEROUS_COMMANDS: &[&str] = &["rm", "mkfs", "dd", "format", "fdisk", "del"];
```

**Rejected Examples**:
- `ls | cat` - Pipe operator
- `echo; rm` - Command separator
- `cat ../../etc/passwd` - Path traversal
- `rm -rf /` - Destructive command

## Configuration

```rust
use agent_sdk::AgentConfig;

let config = AgentConfig::builder()
    .tool_config(ToolConfig {
        enable_shell: true,
        ..Default::default()
    })
    .shell_allowlist(vec![
        "ls".to_string(),
        "cat".to_string(),
        "echo".to_string(),
    ])
    .build()?;
```

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `enable_shell` | `bool` | `false` | Enable shell tool |
| `shell_allowlist` | `Vec<String>` | `[]` | Permitted commands |

## Interface

### WIT Interface

```witninterface tool {
    record tool-info {
        name: string,
        description: string,
        version: string,
        requires-session: bool,
    }

    info: func() -> tool-info;
    execute: func(params: string, session-id: option<string>) -> result<string, string>;
}
```

### Input/Output

**Input**: JSON array `["command", "arg1", "arg2"]`  
**Output**: UTF-8 stdout string (success) or error message

## Usage Examples

### Basic Execution

```rust
// Execute command
let params = json!(["ls", "-la"]);
let result = shell_tool.execute(params.to_string(), None)?;
```

### Agent SDK Usage

```rust
// Register tool
let mut registry = ToolRegistry::new();
registry.register(
    "shell",
    Box::new(ShellTool::new(vec!["ls", "cat"])),
    create_shell_parser(),
);

// Agent uses: <shell>ls -la</shell>
```

### Safe Patterns

```rust
// Good
["ls", "-la"]
["cat", "README.md"]
["echo", "Hello"]
["grep", "pattern", "file.txt"]

// Bad - will be rejected
["ls|cat"]
["cat", "../etc/passwd"]
["rm", "-rf", "/"]
```

## Error Handling

```rust
pub enum ShellError {
    InvalidParams(String),       // Malformed JSON
    NoCommand,                   // Empty parameters
    DangerousCommand(String),   // Metacharacters or blocked command
    ExecutionFailed(String),    // Failed to spawn
    CommandFailed {             // Non-zero exit code
        stdout: String,
        stderr: String,
        code: i32,
    },
    InvalidUtf8(String),        // Binary output
}
```

### Common Errors

| Error | Cause | Resolution |
|-------|-------|------------|
| `DangerousCommand` | Metacharacters detected | Remove shell operators |
| `ExecutionFailed` | Command not found | Verify command installed |
| `CommandFailed` | Non-zero exit code | Check stderr output |

## Best Practices

1. **Always use allowlist** in production
2. **Prefer simple commands** - one operation per call
3. **Never pass untrusted input** to shell commands
4. **Use dedicated tools** when available (Read File Tool, Grep Tool)
5. **Enable audit logging** for security review

### Tool Selection Guide

| Task | Use | Not Use |
|------|-----|---------|
| Read files | Read File Tool | Shell (cat) |
| Search patterns | Grep Tool | Shell (grep) |
| List directories | Shell Tool | - |
| Build commands | Shell Tool | - |

---

**See Also**: [Agent SDK](../agent-sdk.md) | [Read File Tool](read-file-tool.md) | [Grep Tool](grep-tool.md)
