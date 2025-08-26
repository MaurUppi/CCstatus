# Consolidated Debug Logger Enhancement Plan

**Date:** 2025-08-25  
**Target:** Complete rewrite of `src/core/network/debug_logger.rs`  
**Approach:** Unified implementation addressing all identified issues and enhancements

## Design Decisions

### Log Rotation - Simplified
- **Hardcoded at 8MB** (no environment configuration)
- **Gzip compression** with automatic archive cleanup (keep 5 files)
- **File locking** for concurrent process safety
- **Check every 200 writes** for performance

### Environment Variables - Standardized  
- **CCSTATUS_DEBUG only** (values: true, 1, yes, on)
- **No configuration** for rotation limits, compression format, etc.
- **Simplicity over flexibility** for CCometixLine's use case

## Implementation Phases

### Phase 1: Critical Fixes (P0)
**Fixes broken functionality**

- Remove log truncation → append-only logging
- Add synchronous logging methods for process lifecycle
- Implement 8MB rotation with gzip compression  
- Basic JSON Lines format with timestamps

**Effort:** 1 development session  
**Impact:** Restores debugging capability

### Phase 2: Enhanced Structure (P1)
**Production-ready observability**

- Full JSON Lines with timezone-aware timestamps
- Standardize CCSTATUS_DEBUG environment handling
- Add correlation IDs for session tracking
- Implement concurrent-safe rotation with file locking

**Effort:** 1 development session  
**Impact:** Structured, reliable logging

### Phase 3: Safety & Usability (P2)
**Enterprise features**

- Redaction guardrails for sensitive data patterns
- Typed methods for network monitoring events
- Enhanced error handling and reporting
- Performance optimizations

**Effort:** 1 development session  
**Impact:** Production-safe, developer-friendly

## Core Architecture

```rust
// Simplified constants - no configuration
const LOG_ROTATION_SIZE_MB: u64 = 8;
const MAX_ARCHIVES: u32 = 5;
const ROTATION_CHECK_INTERVAL: u32 = 200;

pub struct EnhancedDebugLogger {
    enabled: bool,
    rotating_logger: Option<Arc<Mutex<RotatingLogger>>>,
    session_id: String, // Correlation for this process
}

// JSON Lines structured format
struct LogEntry {
    timestamp: String,              // ISO-8601 with timezone
    level: String,                  // DEBUG, ERROR, PERF, CRED, NETWORK
    component: String,              // Component name  
    event: String,                  // Event type
    message: String,                // Redacted message
    correlation_id: Option<String>, // Session/operation tracking
    fields: Map<String, Value>,     // Structured data
}
```

## Key Methods

### Synchronous Core Methods
```rust  
pub fn debug_sync(&self, component: &str, event: &str, message: &str)
pub fn error_sync(&self, component: &str, event: &str, message: &str)
pub fn performance_sync(&self, component: &str, operation: &str, duration_ms: u64)
```

### Typed Network Monitoring Methods
```rust
pub fn network_probe_start(&self, mode: &str, timeout_ms: u64, correlation_id: String)
pub fn network_probe_end(&self, status: &str, http_status: Option<u16>, duration_ms: u64, correlation_id: String)
pub fn credential_info_safe(&self, source: &str, token_length: usize)
pub fn state_write_summary(&self, status: &str, p95_ms: u64, rolling_window_size: u32)
```

## Redaction System

**Automatic Pattern Detection:**
- Authorization headers and tokens
- Password fields and API keys  
- Suspiciously long strings (>100 chars, no spaces)
- Custom sensitive data patterns

**Safe by Default:**
- All messages automatically scanned
- Only safe credential metadata logged (length, source)
- Typed methods prevent accidental sensitive data logging

## Required Dependencies

```toml
[dependencies]
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
chrono = { version = "0.4", features = ["serde"] }
uuid = { version = "1.0", features = ["v4"] }
flate2 = "1.0"           # Gzip compression
fs2 = "0.4"              # File locking  
regex = "1.0"            # Redaction patterns
```

## File Structure Result

```
~/.claude/ccstatus/
├── ccstatus-debug.log                    # Current JSON Lines log
├── ccstatus-debug.20250825_143022.gz    # Latest compressed archive
├── ccstatus-debug.20250825_120815.gz    # Previous archive
├── ccstatus-debug.20250825_095533.gz    # Older archive
└── ... (maximum 5 total compressed archives)
```

## Performance Characteristics

| Operation | Time Impact | Notes |
|-----------|-------------|-------|
| Normal log write | +0.2ms | JSON serialization + file append |
| Rotation check | +3ms | File size check every 200 writes |
| Full rotation | +100-300ms | Gzip compression, rare (8MB reached) |
| Startup | +1ms | Session ID generation, path setup |

**Total CCometixLine Impact:** <2ms additional startup time

## Success Metrics

✅ **P0 Critical Issues Resolved:**
- No log truncation - persistent debugging across sessions
- Synchronous methods - compatible with short-lived processes  
- Automatic rotation - no manual maintenance required

✅ **P1 Enhanced Functionality:**
- Machine-parseable JSON Lines logs
- Correlation tracking across operations
- Structured observability for network monitoring

✅ **P2 Production Safety:**
- Automatic redaction of sensitive data
- Typed interfaces prevent logging mistakes
- Concurrent-safe multi-process access

## Next Steps

1. **Implement Phase 1** - Fix critical truncation and add basic rotation
2. **Test concurrent access** - Multiple CCometixLine processes 
3. **Implement Phase 2** - JSON Lines and correlation IDs
4. **Add Phase 3 safety** - Redaction and typed methods
5. **Validate network integration** - Test with existing network monitoring components

**Total Implementation Time:** 3 development sessions  
**Testing Time:** 1 session for comprehensive validation  
**Ready for Production:** After Phase 2 completion