### ErrorTracker — Review against Network Monitoring Pipeline v2

Date: 2025-08-25
Target file: `src/core/network/error_tracker.rs`

---

#### Classification Overview
- Critical: 0 findings
- Medium: 0 findings
- Low: 4 findings

---

### Critical
- None

---

### Medium
- None. Previous medium issues mitigated/resolved:
  - RED gating misuse risk: `has_recent_errors()` is now clearly documented as diagnostics-only with 60s window; RED gating remains the responsibility of `JsonlMonitor::scan_tail()` per spec.
  - Non-standard fallback `network_error`: replaced with `unknown_error` in `classify_connection_error` (spec-aligned).
  - Duplicative status determination: method retained but explicitly deprecated to avoid drift.

---

### Low
- Diagnostic-only recent-error window (doc-only guard)
  - Evidence:
```97:105:src/core/network/error_tracker.rs
/// **WARNING**: This method uses a 60-second lookback window which conflicts with 
/// the RED monitoring specification. For RED gating decisions, use JsonlMonitor::scan_tail() 
/// exclusively, which implements the correct 10s/1s frequency window per specification.
pub fn has_recent_errors(&self, current_time_ms: u64) -> bool { /* ... */ }
```
  - Recommendation: Keep as diagnostics-only; optionally rename to `has_recent_errors_diagnostic` to further prevent misuse.

- Duplicative status determination (deprecated)
  - Evidence:
```166:176:src/core/network/error_tracker.rs
#[deprecated(note = "Use HttpMonitor utilities for status determination to maintain single source of truth")]
pub fn determine_status(&self, status_code: u16, latency_ms: u32, p80_threshold: u32, p95_threshold: u32) -> NetworkStatus { /* ... */ }
```
  - Recommendation: Remove or delegate to canonical utilities in `HttpMonitor`/`StatusRenderer` when convenient.

- Percentile utility duplicative (deprecated)
  - Evidence:
```203:206:src/core/network/error_tracker.rs
#[deprecated(note = "Use HttpMonitor utilities for percentile calculations to maintain single source of truth")]
pub fn calculate_percentiles(&self, rolling_totals: &[u32]) -> (u32, u32) { /* ... */ }
```
  - Recommendation: Remove in favor of centralized implementation.

- Configurability enhancement (optional)
  - `max_history` is fixed at 50. Consider exposing a builder or setter for tests and power users. Not required by spec.

---

### Meets Requirements
- Memory-only; does not write state files.
- HTTP status classification aligns with spec (200/429/500/504/529, and 0 → connection_error).
- Diagnostics: latest error retrieval, time-window stats, and cleanup utilities are helpful for observability.
- JSONL timestamp parsing handles fixed-offset ISO-8601 and UTC fallback.

---

### Recommendations (Ordered)
1. Keep `has_recent_errors()` strictly diagnostic; consider renaming to discourage misuse.
2. Remove deprecated `determine_status()` and `calculate_percentiles()` or refactor to delegate to canonical implementations.
3. Optionally expose `max_history` configurability.
