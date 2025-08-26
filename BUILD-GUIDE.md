# CCstatus Build Guide

## Quick Build Options

### Option 1: Default Build (Recommended)
**Foundation + network monitoring for most users**

```bash
# Build release with default features
cargo build --release

# Binary location
target/release/ccstatus  # ~1.8MB
```

**Use when:**
- Standard CCstatus deployment
- Want network monitoring included
- Optimal size/functionality balance
- Most common use case

### Option 2: With Self-Update
**Default build + automatic update notifications**

```bash
# Build with self-update feature
cargo build --release --features "self-update"

# Binary location  
target/release/ccstatus  # ~4.1MB
```

**Use when:**
- Want automatic update notifications
- GitHub releases integration needed
- Prefer guided updates

### Option 3: With TUI Configuration
**Default build + interactive configuration interface**

```bash
# Build with TUI feature
cargo build --release --features "tui"

# Binary location  
target/release/ccstatus  # ~2.5MB
```

**Use when:**
- Want the TUI configurator (`--config`)
- Need theme management
- Interactive setup preferred

### Option 4: Full Build (All Features)
**Complete functionality with everything enabled**

```bash
# Build with all features
cargo build --release --features "tui,self-update"

# Binary location
target/release/ccstatus  # ~4.8MB
```

**Use when:**
- Want all available features
- Development or testing environment
- Maximum functionality needed

### Option 5: Minimal Build
**Absolute minimum (no network monitoring)**

```bash
cargo build --release --no-default-features

# Binary location
target/release/ccstatus  # ~1.5MB (Basic statusline only)
```

**Use when:**
- Smallest possible binary
- No network monitoring needed
- Bare minimum functionality

## Feature Matrix

| Feature | Default | +Self-Update | +TUI | Full | Minimal |
|---------|---------|--------------|------|------|---------|
| Statusline Generation | ✅ | ✅ | ✅ | ✅ | ✅ |
| Network Monitoring | ✅ | ✅ | ✅ | ✅ | ❌ |
| Self-Update | ❌ | ✅ | ❌ | ✅ | ❌ |
| TUI Configurator | ❌ | ❌ | ✅ | ✅ | ❌ |
| Theme Management | ❌ | ❌ | ✅ | ✅ | ❌ |
| Binary Size | ~1.8MB | ~4.1MB | ~2.5MB | ~4.8MB | ~1.5MB |
| Command | `cargo build --release` | `--features "self-update"` | `--features "tui"` | `--features "tui,self-update"` | `--no-default-features` |

## Development Builds

```bash
# Debug build (faster compilation, default features)
cargo build

# Debug with specific features
cargo build --features "tui,self-update"

# With debug symbols
cargo build --profile dev

# Check without building
cargo check --features "tui,self-update"
```

## Testing

```bash
# Run all tests with default features
cargo test --all

# Test specific feature combinations
cargo test --no-default-features --features network-monitoring
cargo test --features "tui,self-update"
cargo test --features "tui,network-monitoring,self-update"

# Network-specific tests only
cargo test network_segment
cargo test http_monitor

# Test minimal build
cargo test --no-default-features
```

## Cross-Compilation

```bash
# For different targets (example: Linux from macOS)
rustup target add x86_64-unknown-linux-gnu

# Default build for target
cargo build --release --target x86_64-unknown-linux-gnu

# Full build for target
cargo build --release --target x86_64-unknown-linux-gnu --features "tui,self-update"
```

## Optimization Tips

### Binary Size
- Use minimal build for smallest size: `--no-default-features` (~1.5MB)
- Default build already optimized: foundation + network (~1.8MB)
- Strip symbols: `cargo build --release && strip target/release/ccstatus`
- Optimize for size: Add to `Cargo.toml`:
  ```toml
  [profile.release]
  lto = true
  codegen-units = 1
  panic = "abort"
  strip = true
  opt-level = "z"  # Optimize for size
  ```

### Compilation Speed  
- Use default build: Excludes heavy TUI and self-update dependencies
- Avoid full build when not needed: TUI adds significant compile time
- Parallel compilation: `cargo build -j $(nproc)`
- Incremental builds: Keep `target/` directory

### Runtime Performance
- Release builds only: `--release` flag is critical
- Target CPU: `RUSTFLAGS="-C target-cpu=native"`

## Dependencies by Feature

### Core (Always Included)
- `serde` + `serde_json`: JSON parsing
- `clap`: Command line interface
- `toml`: Configuration files
- `dirs`: Directory detection  
- `chrono`: Time handling

### Network-Monitoring Feature
- `isahc`: HTTP client
- `tokio`: Async runtime
- `uuid`: Correlation IDs
- `regex`: Pattern matching

### Self-Update Feature
- `ureq`: HTTP client for GitHub API
- `semver`: Semantic versioning comparison

### TUI Feature  
- `ratatui`: Terminal UI framework
- `crossterm`: Cross-platform terminal
- `ansi_term`: ANSI color handling
- `ansi-to-tui`: ANSI to TUI conversion

## Troubleshooting

### Compilation Errors
```bash
# Clean build cache
cargo clean

# Update dependencies  
cargo update

# Check for feature conflicts
cargo tree --features network-monitoring
```

### Missing Dependencies
```bash
# Install Rust if missing
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Update Rust
rustup update
```

### Build Warnings
```bash  
# Fix code warnings
cargo fix --bin ccstatus

# Check specific warnings for different builds
cargo clippy  # Default build
cargo clippy --features "tui,self-update"  # Full build
cargo clippy --no-default-features  # Minimal build
```

### Feature-Specific Issues
```bash
# Test feature combinations
cargo check --features "tui"
cargo check --features "self-update"
cargo check --features "tui,self-update"
cargo check --no-default-features
```