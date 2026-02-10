# Merge Strategies

Brio-Kernel provides sophisticated merge capabilities for combining results from multiple branches. This guide explains each merge strategy and when to use them.

## Overview

When running tasks in parallel branches, you need a way to combine the results. Brio supports four merge strategies:

| Strategy | Description | Best For |
|----------|-------------|----------|
| **Union** | Combines non-conflicting changes, marks conflicts | General purpose, default |
| **Ours** | Prefers base version when conflicts occur | Conservative updates |
| **Theirs** | Prefers branch versions when conflicts occur | Accepting branch changes |
| **Three-Way** | Line-level conflict detection with markers | Precise control |

## Union Strategy (Default)

The **Union** strategy is the most flexible and is used by default.

### How It Works

- Combines changes from all branches
- Detects conflicts when branches modify the same file/location
- Reports conflicts for manual resolution

### Conflict Detection Rules

- Two modifications to the same file = **conflict**
- Deletion + any other change = **conflict**
- Two additions to the same path = **conflict**

### Example

**Base (main.rs):**
```rust
fn main() {
    println!("Hello");
}
```

**Branch A:**
```rust
fn main() {
    println!("Hello, World!");
}
```

**Branch B:**
```rust
fn greet() {
    println!("Hello");
}

fn main() {
    greet();
}
```

**Union Result (clean merge):**
```rust
fn greet() {
    println!("Hello, World!");
}

fn main() {
    greet();
}
```

### When to Use Union

✅ General purpose merging  
✅ When you want to detect all conflicts  
✅ When manual review is required  
✅ Default choice for most workflows

## Ours Strategy

The **Ours** strategy prefers the base version when conflicts occur.

### Example

**Base:**
```rust
fn calculate(x: i32) -> i32 {
    x * 2
}
```

**Branch A:**
```rust
fn calculate(x: i32) -> i32 {
    x * 3  // Changed multiplier
}
```

**Branch B:**
```rust
fn calculate(x: i32) -> i32 {
    x * 2 + 1  // Added offset
}
```

**Ours Result:**
```rust
fn calculate(x: i32) -> i32 {
    x * 2  // Kept base version
}
```

### When to Use Ours

✅ Conservative approach - prefer stability  
✅ Base version is "golden"  
✅ Branches are experimental  
✅ Safety-critical code

## Theirs Strategy

The **Theirs** strategy prefers branch versions when conflicts occur.

### Example

**Base:**
```rust
fn old_function() {
    // Legacy implementation
}
```

**Branch A:**
```rust
fn new_function() {
    // Modern implementation
}
```

**Theirs Result (with Branch A preferred):**
```rust
fn new_function() {
    // Modern implementation
}
```

### When to Use Theirs

✅ Confident in branch changes  
✅ Base is outdated  
✅ Accepting contributions  
✅ Experimental to stable transition

## Three-Way Strategy

The **Three-Way** strategy performs line-level conflict detection using the Myers diff algorithm.

### Conflict Markers

When conflicts are detected, three-way merge generates Git-style markers:

```rust
fn main() {
<<<<<<< HEAD (Base)
    println!("Hello");
||||||| Original
    println!("Hi");
=======
    println!("Greetings");
>>>>>>> Branch-A
}
```

### When to Use Three-Way

✅ Precise control over conflicts  
✅ Familiar Git-style markers  
✅ Manual conflict resolution  
✅ Complex codebases

## Configuration

### Global Strategy

```toml
[supervisor]
merge_strategy = "union"
```

### Per-Branch Strategy

```toml
[[branches]]
name = "experiment-a"
merge_strategy = "theirs"
auto_merge = false

[[branches]]
name = "production-fix"
merge_strategy = "ours"
auto_merge = true
```

## Strategy Comparison

| Scenario | Recommended Strategy |
|----------|---------------------|
| Default/general use | Union |
| Safety-critical, prefer stability | Ours |
| Accepting improvements | Theirs |
| Need precise conflict control | Three-Way |
| Automated merging | Union or Ours |
| Manual review required | Three-Way or Union |

## Best Practices

1. **Start with Union** - Good default for most cases
2. **Use Ours for Safety** - When base is "correct"
3. **Use Theirs for Updates** - When accepting improvements
4. **Use Three-Way for Control** - When you need granularity
5. **Always review conflicts** - Never blindly accept
6. **Test merged code** - Verify functionality after merge
