# Suggested Commands for Development

## Essential Development Commands

### Building and Running
```bash
# Build development version
cargo build

# Build optimized release version
cargo build --release

# Run the application (reads JSON from stdin)
cargo run

# Run with specific theme
cargo run -- --theme default

# Print current configuration
cargo run -- --print

# Enter TUI configuration mode
cargo run -- --config
```

### Code Quality and Testing
```bash
# Run all tests
cargo test

# Run tests with verbose output
cargo test --verbose

# Check code formatting (CI requirement)
cargo fmt -- --check

# Apply code formatting
cargo fmt

# Run clippy linting (CI requirement, treats warnings as errors)
cargo clippy -- -D warnings

# Check code without building
cargo check
```

### Development Utilities
```bash
# Initialize configuration file
cargo run -- --init

# Validate configuration
cargo run -- --check

# Check for available updates
cargo run -- --update
```

### Project Management
```bash
# Clean build artifacts
cargo clean

# Show dependency tree
cargo tree

# Update dependencies
cargo update

# Check for outdated dependencies
cargo outdated
```

### Platform-Specific Commands (Darwin/macOS)
```bash
# Standard Unix commands are available
ls          # List directory contents
find        # Find files
grep        # Search text patterns
git         # Git version control
cd          # Change directory
```

## Entry Points
- **Main binary**: `ccline` (after installation)
- **Development**: `cargo run` 
- **Release binary**: `target/release/ccometixline`

## Configuration Location
- **Config file**: `~/.claude/ccline/config.toml` (when implemented)
- **Install location**: `~/.claude/ccline/ccline`