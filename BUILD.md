## CCstatus Build and Configuration (Concise)

### Targets and Features
- Default features: `network-monitoring`
- Optional features:
  - `tui` (Ratatui-based config UI)
  - `self-update` (update checks)
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
- With self-update:
```bash
cargo build --release --features self-update
```
- With timings (phase metrics via curl):
```bash
cargo build --release --features timings-curl
```
- With static curl timings (bundles libcurl; larger but NO system deps at runtime):
```bash
cargo build --release --features timings-curl-static
```
- Full (UI + updates + timings):
```bash
cargo build --release --features "tui,self-update,timings-curl"
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
- macOS/Linux: `timings-curl` uses system or vendored libcurl; works out of the box.
- Windows: prefer `timings-curl-static` for portable binaries. Expect larger size.

### CI Matrix (example)
```yaml
jobs:
  build:
    runs-on: ${{ matrix.os }}
    strategy:
      matrix:
        os: [ubuntu-latest, macos-latest, windows-latest]
        features: [default, timings-curl]
    steps:
      - uses: actions/checkout@v4
      - uses: actions-rs/toolchain@v1
        with: { toolchain: stable, override: true, profile: minimal }
      - name: Build
        run: |
          if [ "${{ matrix.features }}" = "timings-curl" ]; then
            cargo build --release --features timings-curl
          else
            cargo build --release
          fi
      - name: Test
        run: cargo test --all --release
```

### Tips
- Size: use `--no-default-features` and strip symbols for smallest footprint.
- Speed: `RUSTFLAGS="-C target-cpu=native -C opt-level=3" cargo build --release`.
- Diagnose feature sets: `cargo tree --features "tui,self-update,timings-curl"`.

### Troubleshooting
- Clean cache on odd errors: `cargo clean && cargo build --release`.
- Validate features compile: `cargo check --no-default-features` and `cargo check --features timings-curl`.
- If curl runner errors during runtime, the monitor will fall back to isahc heuristic timings when configured.
