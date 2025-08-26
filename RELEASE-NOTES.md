# CCstatus v1.0.4 - Network Monitoring Release

## 🚀 New Features

### Window-Based Network Deduplication
- **Per-window deduplication**: GREEN and RED window probes now track window IDs to prevent duplicate probes within the same window
- **Execution-based persistence**: Window IDs are only persisted after successful probe execution
- **Monotonic updates**: Window ID persistence uses atomic writes with monotonic checks

### Single JSONL Scan Optimization
- Eliminated duplicate I/O by scanning transcript once before window calculation
- Pre-computed error detection results shared between window decision and probe execution
- Significant performance improvement for RED window error detection

### COLD State Validation Enhancements  
- Enhanced COLD probe logic to check both time window AND state validity
- Proper session deduplication to prevent multiple COLD probes for same session
- Improved startup probe reliability

### Code Quality Improvements
- Eliminated code duplication in COLD window threshold logic
- Fixed environment variable precedence bug (`CCSTATUS_COLD_WINDOW_MS` > `ccstatus_COLD_WINDOW_MS`)
- Comprehensive test coverage for all enhancement phases (45+ tests passing)

## 🔧 Build Options

### Network-Only Release (Recommended)
For users who only need network monitoring without TUI configuration:

```bash
cargo build --release --no-default-features --features network-monitoring
```

**Benefits:**
- **28% smaller binary**: 1.87MB vs 2.60MB
- **Faster compilation**: Excludes TUI dependencies (ratatui, crossterm, ansi-to-tui)
- **Focused functionality**: Pure network monitoring and statusline generation

### Full Release (TUI + Network)
For users who want both network monitoring and TUI configuration:

```bash
cargo build --release
# or
cargo build --release --features "tui,network-monitoring"
```

**Includes:**
- Full TUI configuration interface (`ccstatus --config`)
- Theme management and customization
- Interactive segment configuration
- All network monitoring features

## 📊 Performance Improvements

- **Single I/O Operations**: Eliminated duplicate JSONL transcript scanning
- **Atomic State Writes**: Temporary file + rename pattern for safe state persistence  
- **Memory Efficiency**: Window ID deduplication prevents unnecessary probe executions
- **Binary Size**: Network-only build is 28% smaller (1.87MB vs 2.60MB)

## 🧪 Testing

- **45 tests passing** including comprehensive integration tests
- **All enhancement phases covered**: Single scan, deduplication, COLD validation
- **Window priority testing**: COLD > RED > GREEN logic verification
- **Edge case coverage**: Zero duration, large values, boundary conditions

## ⚙️ Configuration

### Environment Variables
- `CCSTATUS_COLD_WINDOW_MS` / `ccstatus_COLD_WINDOW_MS`: COLD window threshold (default: 5000ms)
- `CCSTATUS_TIMEOUT_MS`: Network timeout override (max 6000ms)
- `CCSTATUS_DEBUG`: Debug logging enable (true/1/yes/on)

### Network Monitoring
- **COLD Window**: Startup probes (< 5000ms) with session deduplication
- **RED Window**: Error-driven probes (10s intervals, first 1s of window)  
- **GREEN Window**: Regular health probes (300s intervals, first 3s of window)

## 📝 Compatibility

- **Rust**: 1.88.0+
- **Tokio**: Multi-threaded async runtime
- **HTTP Client**: isahc with JSON support
- **Backwards Compatible**: All existing configurations supported

## 🚦 Upgrade Path

1. **From 1.0.3**: Direct upgrade, no breaking changes
2. **New Installations**: Choose build variant based on needs
3. **TUI Users**: Use full build to retain configuration interface
4. **CI/Automation**: Consider network-only build for smaller deployments

## 📈 Next Steps

- Consider binary distribution for both variants
- Performance benchmarking against previous versions
- Integration testing in production Claude Code environments