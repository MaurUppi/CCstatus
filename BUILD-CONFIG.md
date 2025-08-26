# CCstatus Build Configuration Guide

This document explains the modular build system and feature configuration options for CCstatus.

## Quick Reference

| Build Type | Command | Features | Size | Use Case |
|------------|---------|----------|------|----------|
| **Default** | `cargo build --release` | Foundation + Network | ~1.8MB | Recommended for most users |
| **Network Only** | `cargo build --release --no-default-features --features network-monitoring` | Basic + Network | ~1.8MB | Same as default, explicit |
| **+ Self-Update** | `cargo build --release --features self-update` | Default + Updates | ~4.1MB | Auto-update notifications |
| **+ TUI** | `cargo build --release --features tui` | Default + Config UI | ~2.5MB | Interactive configuration |
| **Full** | `cargo build --release --features "tui,self-update"` | All features | ~4.8MB | Complete functionality |
| **Minimal** | `cargo build --release --no-default-features` | Foundation only | ~1.5MB | Bare minimum |

## Feature Architecture

CCstatus uses a modular architecture with the following feature flags:

### Core Features

#### `default = ["network-monitoring"]`
The default configuration provides essential functionality with network monitoring:
- **Foundation**: Core statusline generation, Git integration, model display, usage tracking
- **Network Monitoring**: Real-time Claude API connectivity status
- **Size**: ~1.8MB
- **Target Users**: All users wanting network monitoring without extra features

#### `network-monitoring = ["isahc", "tokio"]`
Enables real-time Claude API connectivity monitoring:
- **Dependencies**: HTTP client (isahc), async runtime (tokio)
- **Functionality**: COLD/GREEN/RED monitoring windows, P95 latency tracking, credential detection
- **API Impact**: < 1 call per 5 minutes (frequency-gated)
- **State**: Persistent across sessions
- **Debug**: `CCSTATUS_DEBUG=true` support

### Optional Features

#### `tui = ["ratatui", "crossterm", "ansi_term", "ansi-to-tui"]`
Terminal User Interface for interactive configuration:
- **Dependencies**: TUI framework, terminal handling, color processing
- **Functionality**: Real-time statusline preview, theme switching, configuration editing
- **Size Impact**: +0.7MB over default
- **Use Case**: Users who want graphical configuration interface

#### `self-update = ["ureq", "semver"]`
Automatic update checking and notifications:
- **Dependencies**: HTTP client (ureq), semantic versioning
- **Functionality**: GitHub release checking, version comparison, update notifications
- **Size Impact**: +2.3MB over default (includes HTTP client overhead)
- **Update Frequency**: Every hour (configurable)
- **User Control**: Manual update execution required

## Build Commands

### Development Builds
```bash
# Fast compilation for development
cargo build --features network-monitoring

# Check compilation without building
cargo check --features "tui,self-update"

# Run with specific features
cargo run --features "tui" -- --help
```

### Release Builds
```bash
# Default optimized build
cargo build --release

# Specific feature combinations
cargo build --release --features "self-update"
cargo build --release --features "tui"
cargo build --release --features "tui,self-update"

# Minimal build (no network monitoring)
cargo build --release --no-default-features
```

### Cross-Platform Builds
```bash
# Linux static (universal compatibility)
cargo build --release --target x86_64-unknown-linux-musl

# Windows from Unix
cargo build --release --target x86_64-pc-windows-gnu

# macOS universal binary (requires setup)
cargo build --release --target universal-apple-darwin
```

## Feature Combinations

### Recommended Configurations

#### 1. **Standard User** (Default)
```bash
cargo build --release
# Features: foundation + network-monitoring
# Size: ~1.8MB
# Perfect for: Claude Code integration, basic monitoring needs
```

#### 2. **Power User with Updates**
```bash
cargo build --release --features "self-update"
# Features: foundation + network-monitoring + self-update
# Size: ~4.1MB
# Perfect for: Users wanting automatic update notifications
```

#### 3. **Configuration Enthusiast**
```bash
cargo build --release --features "tui"
# Features: foundation + network-monitoring + tui
# Size: ~2.5MB
# Perfect for: Users who want interactive configuration
```

#### 4. **Complete Installation**
```bash
cargo build --release --features "tui,self-update"
# Features: All available features
# Size: ~4.8MB
# Perfect for: Full-featured deployment, developers
```

### Specialized Configurations

#### Minimal Installation
```bash
cargo build --release --no-default-features
# Features: Foundation only (no network monitoring)
# Size: ~1.5MB
# Use case: Minimal statusline without API monitoring
```

#### Network Only (Explicit)
```bash
cargo build --release --no-default-features --features network-monitoring
# Features: Foundation + network monitoring (same as default)
# Size: ~1.8MB
# Use case: Explicit about wanting only network features
```

## Environment Variables

Build-time configurations:

### Optimization
```bash
# Optimize for size
RUSTFLAGS="-C opt-level=z" cargo build --release

# Optimize for speed  
RUSTFLAGS="-C opt-level=3 -C target-cpu=native" cargo build --release
```

### Feature Testing
```bash
# Enable all debug features
CCSTATUS_DEBUG=true cargo run --features "tui,self-update"

# Network monitoring debug
CCSTATUS_DEBUG=true cargo run --features network-monitoring
```

## Size Analysis

### Component Breakdown
- **Foundation** (Git, model, usage, directory): ~1.5MB
- **Network Monitoring** (isahc + tokio): +0.3MB
- **TUI** (ratatui + crossterm + colors): +0.7MB  
- **Self-Update** (ureq + semver): +2.3MB

### Optimization Tips
1. **Use default build** for most deployments
2. **Strip symbols**: `cargo build --release && strip target/release/ccstatus`
3. **Size optimization**: Add to Cargo.toml:
   ```toml
   [profile.release]
   lto = true
   codegen-units = 1
   panic = "abort"
   strip = true
   opt-level = "z"  # Optimize for size
   ```

## Dependency Management

### Core Dependencies (Always Present)
- `serde`, `serde_json`: Configuration and data serialization
- `clap`: Command-line interface
- `chrono`: Time handling
- `dirs`: Directory detection
- `regex`, `uuid`, `flate2`, `fs2`: Utility functions

### Feature-Specific Dependencies
- **network-monitoring**: `isahc`, `tokio`
- **tui**: `ratatui`, `crossterm`, `ansi_term`, `ansi-to-tui`
- **self-update**: `ureq`, `semver`

### Version Management
All dependencies are pinned to specific versions for reproducible builds:
- Major updates are tested for compatibility
- Security updates are applied promptly
- Feature flags isolate dependency changes

## Troubleshooting

### Common Issues

#### Feature Compilation Errors
```bash
# Clear cache and rebuild
cargo clean
cargo build --release --features "your-features"

# Check dependency conflicts
cargo tree --features "tui,self-update"
```

#### Missing Feature Warnings
```rust
// Code is feature-gated, warnings are normal when features disabled
warning: function `visible_width` is never used
```

#### Size Concerns
- Default build is optimized for functionality/size balance
- Use `--no-default-features` for absolute minimum
- Consider static linking for deployment: `--target x86_64-unknown-linux-musl`

### Debugging Build Issues
```bash
# Verbose build output
cargo build --release --verbose

# Check feature resolution
cargo metadata --format-version 1 | grep features

# Validate feature flags
cargo check --all-features
cargo check --no-default-features
```

## Migration Guide

### From v1.0.3 to v1.0.4
- **Default features changed**: TUI is now optional
- **New build required**: Previous builds won't include network monitoring
- **Size reduction**: Default build is 30% smaller
- **Compatibility**: All functionality still available via feature flags

### Upgrade Commands
```bash
# Before (v1.0.3): Default included TUI
cargo build --release  # ~2.6MB

# After (v1.0.4): TUI is optional
cargo build --release  # ~1.8MB (network only)
cargo build --release --features tui  # ~2.5MB (with TUI)
```

## CI/CD Integration

### GitHub Actions Example
```yaml
- name: Build default
  run: cargo build --release

- name: Build all features  
  run: cargo build --release --features "tui,self-update"

- name: Test feature combinations
  run: |
    cargo test --no-default-features
    cargo test --features network-monitoring
    cargo test --features "tui,self-update"
```

### Docker Multi-Stage Builds
```dockerfile
# Build stage with full features
FROM rust:1.70 as builder
COPY . .
RUN cargo build --release --features "tui,self-update"

# Runtime stage with minimal image
FROM debian:bookworm-slim
COPY --from=builder /target/release/ccstatus /usr/local/bin/
```

This configuration system provides maximum flexibility while maintaining simplicity for common use cases.