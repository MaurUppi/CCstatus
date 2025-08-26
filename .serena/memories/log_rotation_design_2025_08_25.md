# Log Rotation with Compression - Implementation Design

**Date:** 2025-08-25  
**Context:** Enhancement for debug_logger.rs to implement append-only logging with automatic rotation and compression

## Design Overview

**Strategy:** Intelligent append-only logging with atomic rotation when size/line thresholds exceeded.

**Key Features:**
- Append-only (no truncation) with automatic rotation
- Dual trigger: file size OR line count
- Gzip/Zip compression with atomic operations
- Concurrent process safety via file locking
- Automatic cleanup of old archives
- Configurable via environment variables

## Configuration Parameters

```rust
pub struct LogRotationConfig {
    pub max_size_mb: u64,        // Default: 10MB
    pub max_lines: u64,          // Default: 50,000 lines
    pub max_archives: u32,       // Default: 5
    pub compression_format: CompressionFormat, // Default: Gzip
    pub check_interval: u32,     // Default: 100 writes
}
```

## Environment Variables

```bash
export CCSTATUS_LOG_MAX_MB=10          # Size trigger
export CCSTATUS_LOG_MAX_LINES=50000    # Line count trigger  
export CCSTATUS_LOG_MAX_ARCHIVES=5     # Archive retention
export CCSTATUS_DEBUG=1                # Enable logging
```

## Atomic Rotation Process

1. **Check Triggers:** Every N writes, check size/line thresholds
2. **Acquire Lock:** Use file locking to prevent concurrent rotation
3. **Atomic Move:** Rename current log to temporary file
4. **Compress:** Create compressed archive with timestamp
5. **Cleanup:** Remove temporary file and old archives
6. **Release:** Lock automatically released

## File Locking Strategy

```rust
// Advisory locking prevents concurrent rotation
let lock_file = OpenOptions::new().create(true).write(true).open(&lock_path)?;
match lock_file.try_lock_exclusive() {
    Ok(()) => { /* perform rotation */ }
    Err(_) => { /* another process rotating, skip */ }
}
```

## Compression Implementation

**Gzip (Recommended):**
- Better compression ratio (~70% reduction)
- Faster compression/decompression
- Standard tooling support

**Zip Alternative:**
- Cross-platform compatibility
- Built-in directory support
- GUI tool integration

## Error Handling Strategy

**Graceful Degradation:**
- Rotation failures don't stop logging
- Silent fallback to continued append
- Optional error reporting to stderr

**Error Types:**
- I/O errors during rotation
- Compression failures
- Lock acquisition timeout
- Archive cleanup failures

## Performance Characteristics

| Operation | Time Impact | Frequency |
|-----------|-------------|-----------|
| Normal write | +0.1ms | Every log |
| Rotation check | +2-5ms | Every 100 writes |
| Full rotation | +50-200ms | Rare (10MB) |

## Required Dependencies

```toml
[dependencies]
flate2 = "1.0"           # Gzip compression
zip = "0.6"              # Zip compression (optional)
fs2 = "0.4"              # File locking
chrono = "0.4"           # Timestamps (existing)
```

## File Structure Example

```
~/.claude/ccstatus/
├── ccstatus-debug.log              # Active log
├── ccstatus-debug.20250825_143022.gz  # Latest archive
├── ccstatus-debug.20250825_120815.gz  # Previous archive
└── ccstatus-debug.lock             # Rotation lock
```

## Testing Requirements

1. **Rotation Triggers:** Size and line count thresholds
2. **Concurrent Safety:** Multiple processes writing simultaneously  
3. **Compression Integrity:** Verify archived logs decompress correctly
4. **Cleanup Logic:** Archive retention limits respected
5. **Error Scenarios:** Disk full, permission errors, corruption

## Integration Points

**With Existing DebugLogger:**
- Replace log truncation with rotation checks
- Add rotation config to constructor
- Integrate rotation with sync/async write methods

**With CCometixLine:**
- Minimal startup time impact (<1ms)
- No changes to existing logging API
- Environment-based configuration

## Benefits Delivered

✅ **Zero Data Loss** - True append-only behavior
✅ **Storage Efficient** - 70% compression reduces disk usage  
✅ **Concurrent Safe** - Multiple processes can log safely
✅ **Performance Optimized** - Minimal impact on statusline speed
✅ **Operationally Friendly** - Automatic maintenance and cleanup
✅ **Configurable** - Tunable thresholds via environment variables
✅ **Robust** - Graceful degradation on failures

## Implementation Priority

**P1 (High):** Core rotation with gzip compression and file locking
**P2 (Medium):** Archive cleanup and configuration via environment  
**P3 (Low):** Zip format support and enhanced error reporting

**Estimated Effort:** 1-2 development sessions for P1, additional session for P2-P3
**Testing Time:** 1 session for comprehensive concurrent testing

This design solves the critical log truncation issue while adding enterprise-grade log management suitable for network monitoring operations.