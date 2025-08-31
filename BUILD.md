## CCstatus Build and Configuration (Concise)

### Targets and Features
- Default features: `["network-monitoring", "self-update"]`
- Optional features:
  - `tui` (Ratatui-based config UI)
  - `timings-curl` (DNS/TCP/TLS/TTFB via libcurl; auto-wired runner)
  - `timings-curl-static` (static curl; primarily for Windows/Linux portability)

### Common Build Commands
- Default (recommended):
```bash
cargo build --release
```
- With TUI:
```bash
cargo build --release --features tui
```
- Network monitoring only (without self-update):
```bash
cargo build --release --features network-monitoring --no-default-features
```
- With timings (phase metrics via curl):
```bash
cargo build --release --features timings-curl
```
- With static curl timings (bundles libcurl; larger but NO system deps at runtime):
```bash
cargo build --release --features timings-curl-static
```
- Full (UI + timings with default self-update):
```bash
cargo build --release --features "tui,timings-curl"
```
- Minimal (no defaults):
```bash
cargo build --release --no-default-features
```

### Testing
- Default tests:
```bash
cargo test --release
```
- Network-only tests explicitly:
```bash
cargo test --no-default-features --features network-monitoring
```
- With timings-curl enabled (unit tests should inject FakeCurlRunner):
```bash
cargo test --release --features timings-curl
```

### Platform Notes
- **macOS**: `timings-curl-static` recommended for universal compatibility (fixes ARM64 OpenSSL path issues)
- **Linux**: Both `timings-curl` and `timings-curl-static` work; static builds eliminate glibc dependencies
- **Windows**: `timings-curl-static` required for portable binaries without runtime dependencies

### Binary Size Optimization

**Size Comparison (ARM64 macOS)**:
- **Slim** (`network-monitoring,self-update`): ~3.2MB (requires system OpenSSL 3.x)
- **Static** (`timings-curl-static,self-update`): ~6.7MB (fully static, zero dependencies)

**Optimization Strategies**:
```bash
# Minimal size with system dependencies (excludes self-update, may have OpenSSL path issues)
cargo build --release --features network-monitoring --no-default-features

# Balanced: static linking with size optimization (includes self-update by default)
RUSTFLAGS="-C opt-level=z -C codegen-units=1 -C panic=abort" \
OPENSSL_STATIC=1 \
cargo build --release --features timings-curl-static

# Development: fastest build time (excludes self-update)
cargo build --features network-monitoring --no-default-features
```

**Distribution Strategy**:
- **Static builds** (`timings-curl-static,self-update`): Universal compatibility, no system dependencies (recommended)
- **Slim builds** (`network-monitoring,self-update`): Smaller size, requires `brew install openssl@3` on macOS

### CI Matrix (actual)
```yaml
jobs:
  build:
    strategy:
      matrix:
        include:
          - target: x86_64-unknown-linux-gnu
            os: ubuntu-22.04
            variant: static
          - target: x86_64-pc-windows-msvc
            os: windows-2022
            variant: static
          - target: x86_64-apple-darwin
            os: macos-13
            variant: static
          - target: x86_64-apple-darwin
            os: macos-13
            variant: slim
          - target: aarch64-apple-darwin
            os: macos-15
            variant: static
          - target: aarch64-apple-darwin
            os: macos-15
            variant: slim
    steps:
      # Configure static linking for macOS
      - name: Configure static linking for macOS
        if: runner.os == 'macOS'
        run: |
          echo "OPENSSL_STATIC=1" >> $GITHUB_ENV
          echo "OPENSSL_NO_VENDOR=1" >> $GITHUB_ENV
      
      # Build based on variant
      - name: Build binary  
        run: |
          if [ "${{ matrix.variant }}" = "slim" ]; then
            cargo build --release --target ${{ matrix.target }} --features "network-monitoring,self-update"
          else
            cargo build --release --target ${{ matrix.target }} --features "timings-curl-static,self-update"
          fi
```

### Tips
- Size: use `--no-default-features` and strip symbols for smallest footprint.
- Speed: `RUSTFLAGS="-C target-cpu=native -C opt-level=3" cargo build --release`.
- Diagnose feature sets: `cargo tree --features "tui,self-update,timings-curl"`.

### Troubleshooting
- Clean cache on odd errors: `cargo clean && cargo build --release`.
- Validate features compile: `cargo check --no-default-features` and `cargo check --features timings-curl`.
- If curl runner errors during runtime, the monitor will fall back to isahc heuristic timings when configured.
