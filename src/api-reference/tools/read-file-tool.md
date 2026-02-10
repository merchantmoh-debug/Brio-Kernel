# Read File Tool API Reference

The Read File Tool is a WASM component for safe file reading with line range support.

## Overview

Read entire files or specific line ranges with size limits and path validation.

**Use Cases**: Reading source code, configs, documentation  
**Line Ranges**: Useful for large files, log reading, targeted code review

## Features

### File Size Limits

```rustnconst MAX_FILE_SIZE: u64 = 10 * 1024 * 1024;  // 10 MB
```

### Line Range Limits

```rustnconst MAX_LINES: u32 = 10_000;  // Max range size
```

### Line Numbering

1-indexed, inclusive:

```rustn// Read lines 1-50
read_file_range("file.rs", 1, 50)?;
```

## Interface

### WIT Definition

```witninterface tool-read-file {
    read-file: func(path: string) -> result<string, string>;
    read-file-range: func(path: string, start-line: u32, end-line: u32) -> result<string, string>;
}
```

### Parameters

**read_file**:
| Parameter | Type | Constraints |
|-----------|------|-------------|
| `path` | `string` | Relative, no traversal, < 10MB |

**read_file_range**:
| Parameter | Type | Constraints |
|-----------|------|-------------|
| `path` | `string` | Relative, no traversal |
| `start-line` | `u32` | >= 1 |
| `end-line` | `u32` | >= start-line, range <= 10,000 |

### Output

**Success**: UTF-8 file contents  
**Error**: Error message string

## Security

### Path Validation

```rustn// Rejected
"../etc/passwd"
"/etc/passwd"
"..\\windows\\system32"
"file.txt\0.exe"

// Accepted
"src/main.rs"
"README.md"
```

### Checks

| Check | Purpose |
|-------|---------|
| Path traversal | Prevent access outside working dir |
| Absolute paths | Restrict to relative only |
| Null bytes | Prevent injection |
| File size | Prevent memory exhaustion |
| Line limits | Prevent excessive memory use |

## Usage Examples

### Reading Entire File

```rustnlet content = read_file_tool.read_file("src/main.rs".to_string())?;
```

### Agent SDK Usage

```rustnregistry.register_read_file_tool();

// Agent uses: <read_file path="src/main.rs" />
// Or with range: <read_file path="src/main.rs" offset="1" limit="50" />
```

### Line Range Operations

```rustn// Read file header (first 20 lines)
let header = read_file_tool.read_file_range("src/lib.rs", 1, 20)?;

// Read specific function
let func = read_file_tool.read_file_range("src/lib.rs", 100, 150)?;
```

### Safe Reading Pattern

```rustnfn read_source(path: &str) -> Result<String, String> {
    // Additional client-side validation
    if path.contains("..") || path.starts_with('/') {
        return Err("Invalid path".to_string());
    }
    
    read_file_tool.read_file(path.to_string())
}
```

### Pagination

```rustnfn read_paginated(path: &str, page: u32, size: u32) -> Result<String, String> {
    let start = (page * size) + 1;
    let end = start + size - 1;
    read_file_tool.read_file_range(path.to_string(), start, end)
}

let page1 = read_paginated("large.txt", 0, 100)?;  // Lines 1-100
let page2 = read_paginated("large.txt", 1, 100)?;  // Lines 101-200
```

## Error Handling

```rustnpub enum ReadFileError {
    InvalidPath(String),       // Traversal, absolute, null bytes
    FileNotFound(String),      // Missing file
    PermissionDenied(String),  // No access
    IoError(String),          // Read error
    FileTooLarge(u64),        // > 10MB
    InvalidLineRange { start: u32, end: u32 },
    LineLimitExceeded(u32),   // > 10,000 lines
}
```

| Error | Cause | Resolution |
|-------|-------|------------|
| `FileTooLarge` | File > 10MB | Use line ranges |
| `InvalidLineRange` | Invalid start/end | Ensure start >= 1, end >= start |
| `LineLimitExceeded` | Range > 10,000 | Use smaller ranges |

---

**See Also**: [Agent SDK](../agent-sdk.md) | [Grep Tool](grep-tool.md) | [Shell Tool](shell-tool.md)
