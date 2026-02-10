# Installation

This guide will help you install Brio-Kernel and its dependencies.

## Prerequisites

Before installing Brio, ensure you have the following:

| Requirement | Version | Purpose |
|-------------|---------|---------|
| Rust | 1.93+ | Core runtime and components |
| wasm-tools | Latest | WIT interface compilation |
| wasm32-wasi target | Latest | WASM component target |
| SQLite | 3.x | State management |
| Git | 2.x | Version control |

### Installing Rust

```bash
# Install Rust via rustup
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env

# Verify installation
rustc --version  # Should be 1.93 or higher
```

### Installing WASM Tools

```bash
# Install wasm-tools
cargo install wasm-tools

# Add WASM target
rustup target add wasm32-wasi
```

### Installing GitHub CLI (Optional)

For easier PR workflows:

```bash
# macOS
brew install gh

# Ubuntu/Debian
curl -fsSL https://cli.github.com/packages/githubcli-archive-keyring.gpg | sudo dd of=/usr/share/keyrings/githubcli-archive-keyring.gpg
sudo chmod go+r /usr/share/keyrings/githubcli-archive-keyring.gpg
echo "deb [arch=$(dpkg --print-architecture) signed-by=/usr/share/keyrings/githubcli-archive-keyring.gpg] https://cli.github.com/packages stable main" | sudo tee /etc/apt/sources.list.d/github-cli.list > /dev/null
sudo apt update
sudo apt install gh
```

## Installing Brio-Kernel

### From Source

```bash
# Clone the repository
git clone https://github.com/Brio-Kernel/brio-kernel.git
cd brio-kernel

# Build the entire workspace
cargo build --workspace

# Run tests to verify installation
cargo test --workspace
```

### Building WASM Components

Brio uses WebAssembly components for agents and tools:

```bash
# Build all WASM components
cd brio-core

# Build individual components
cargo build -p supervisor --target wasm32-wasi
cargo build -p coder-agent --target wasm32-wasi
cargo build -p smart-agent --target wasm32-wasi

# Build all agents
for agent in coder council foreman reviewer smart-agent; do
    cargo build -p ${agent}-agent --target wasm32-wasi
done

# Build tools
cargo build -p shell-tool --target wasm32-wasi
cargo build -p tool-grep --target wasm32-wasi
cargo build -p tool-read-file --target wasm32-wasi
```

## Verification

After installation, verify everything works:

```bash
# Check kernel binary
cargo run --bin brio-kernel -- --version

# Check component compilation
ls brio-core/target/wasm32-wasi/debug/*.wasm

# You should see:
# - supervisor.wasm
# - coder_agent.wasm
# - smart_agent.wasm
# - shell_tool.wasm
# - tool_grep.wasm
# - tool_read_file.wasm
```

## Docker Installation (Optional)

If you prefer using Docker:

```bash
# Build Docker image
docker build -t brio-kernel .

# Run container
docker run -it --rm -p 8080:8080 brio-kernel
```

> **Note:** Docker support is experimental. Native installation is recommended for development.

## Next Steps

- [Quick Start Guide](quickstart.md) - Run your first Brio workflow
- [Configuration](configuration.md) - Customize Brio for your needs
- [Architecture Overview](../concepts/architecture.md) - Understand how Brio works

## Troubleshooting

### Common Issues

**Issue:** `wasm32-wasi target not found`

**Solution:**
```bash
rustup target add wasm32-wasi
```

**Issue:** `wasm-tools: command not found`

**Solution:**
```bash
cargo install wasm-tools
```

**Issue:** Build fails with linking errors

**Solution:**
```bash
# Install build dependencies (Ubuntu/Debian)
sudo apt-get update
sudo apt-get install -y build-essential pkg-config libssl-dev

# Clean and rebuild
cargo clean
cargo build --workspace
```

**Issue:** SQLite errors during tests

**Solution:**
```bash
# Ensure SQLite is installed
sqlite3 --version

# For macOS
brew install sqlite

# For Ubuntu/Debian
sudo apt-get install libsqlite3-dev
```

Still having issues? Check the [Troubleshooting Guide](../reference/troubleshooting.md) or [open an issue](https://github.com/Brio-Kernel/brio-kernel/issues).
