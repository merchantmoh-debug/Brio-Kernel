# Grep Tool API Reference

The Grep Tool is a WASM component that provides safe pattern matching for searching file contents.

## Overview

Performs substring matching without regex evaluation, returning matches with line numbers and content.

**Use Cases**: Finding function definitions, TODO comments, imports, code navigation  
**Pattern Type**: Case-sensitive substring search (not regex)

## Features

### Pattern Matching

Simple substring matching:

```rust
pattern: "fn main"
line: "fn main() {"
// -> Match found at line 42
```

### Limits

- **Pattern length**: Max 1000 characters
- **No regex**: Literal substring only

### Result Structure

```witnrecord grep-match {
    line-number: u32,
    content: string,
}

record grep-result {
    file-path: string,
    matches: list<grep-match>,
}
```

## Interface

### WIT Definition

```witninterface tool-grep {
    grep: func(pattern: string, path: string) -> result<list<grep-result>, string>;
}
```

### Parameters

| Parameter | Type | Constraints |
|-----------|------|-------------|
| `pattern` | `string` | 1-1000 chars, non-empty |
| `path` | `string` | Relative path, no traversal |

### Output

**Success**: Array of `GrepResult` structs with file path and matches  
**Error**: Error message string

## Security

### Path Validation

```rust
// Rejected
"../etc/passwd"
"/etc/passwd"
"..\\windows\\system32"
"file.txt\0.exe"

// Accepted
"src/main.rs"
"docs/api.md"
```

### Pattern Validation

- Empty patterns rejected
- Patterns > 1000 chars rejected

## Usage Examples

### Basic Search

```rust
let results = grep_tool.grep(
    "fn ".to_string(),
    "src/main.rs".to_string()
)?;
```

### Agent SDK Usage

```rust
// Register tool
registry.register_grep_tool();

// Agent uses: <search pattern="fn main" path="src/main.rs" />
```

### Processing Results

```rust
match grep_tool.grep(pattern, path) {
    Ok(results) => {
        for result in results {
            for m in result.matches {
                println!("Line {}: {}", m.line_number, m.content);
            }
        }
    }
    Err(e) => eprintln!("Search failed: {}", e),
}
```

### Common Patterns

```rust
// Find function definitions
let pattern = "fn ";

// Find struct definitions
let pattern = "struct ";

// Find TODO comments
let pattern = "TODO";

// Find specific imports
let pattern = "use std::io";
```

## Performance

- **Streaming**: Line-by-line processing
- **Complexity**: O(n×m) where n=file size, m=pattern length
- **Memory**: Proportional to line length, not file size
- **No size limit**: Unlike Read File Tool

**Recommendations**:
- Use specific, shorter patterns
- Batch searches on related files
- For very large files, consider Read File Tool with ranges

## Error Cases

```rust
pub enum GrepError {
    InvalidPath(String),       // Traversal or null bytes
    FileNotFound(String),      // File doesn't exist
    PermissionDenied(String),  // Can't read file
    IoError(String),          // Read error
    InvalidPattern(String),   // Empty or too long
}
```

| Error | Cause | Resolution |
|-------|-------|------------|
| `InvalidPath` | Path traversal | Use relative paths |
| `FileNotFound` | Missing file | Verify path |
| `InvalidPattern` | Empty/oversized | Use valid pattern (1-1000) |

---

**See Also**: [Agent SDK](../agent-sdk.md) | [Read File Tool](read-file-tool.md) | [Shell Tool](shell-tool.md)
