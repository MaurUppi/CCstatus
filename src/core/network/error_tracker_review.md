### ErrorTracker — Review against Network Monitoring Pipeline v2

Date: 2025-08-25
Target file: `src/core/network/error_tracker.rs`

---

#### Classification Overview
- Critical: 0 findings
- Medium: 3 findings
- Low: 3 findings

---

### Critical
- None

---

### Medium
- Potential misuse for RED gating (window mismatch)
  - Purpose per spec: RED triggered strictly by JsonlMonitor detecting `isApiErrorMessage:true` and then a 10s/1s frequency window. `ErrorTracker::has_recent_errors()` uses a fixed 60s lookback which conflicts with the spec and invites misuse if called by gating logic.
  - Evidence:
```95:101:src/core/network/error_tracker.rs
pub fn has_recent_errors(&self, current_time_ms: u64) -> bool {
    let cutoff_time = current_time_ms.saturating_sub(60_000);
    self.error_history.iter().any(|event|
        event.timestamp >= cutoff_time && event.timestamp <= current_time_ms)
}
```
  - Recommendation: Keep for diagnostics only; do not use for gating. Add doc comment warning and ensure callers use `JsonlMonitor` for RED.

- Non-standard error type name "network_error"
  - Spec declares standardized `error_type` values; connection problems should be `connection_error` or `unknown_error`. Returning `network_error` introduces a third, undocumented label.
  - Evidence:
```133:145:src/core/network/error_tracker.rs
pub fn classify_connection_error(error_message: &str) -> String {
    let message_lower = error_message.to_lowercase();
    if message_lower.contains("connection error") ||
       message_lower.contains("fetch failed") ||
       message_lower.contains("network error") ||
       message_lower.contains("timeout") ||
       message_lower.contains("connection refused") {
        "connection_error".to_string()
    } else {
        "network_error".to_string()
    }
}
```
  - Recommendation: Replace fallback with `unknown_error` to align with the requirement.

- Duplicative status determination risks divergence
  - `determine_status()` re-implements P80/P95 and HTTP mapping that the spec assigns to `HttpMonitor`/`StatusRenderer` as the single source of truth. Keeping divergent logic can cause inconsistency over time.
  - Evidence:
```147:173:src/core/network/error_tracker.rs
pub fn determine_status(&self, status_code: u16, latency_ms: u32, p80_threshold: u32, p95_threshold: u32) -> NetworkStatus { /* ... */ }
```
  - Recommendation: Either centralize this in `HttpMonitor`/`StatusRenderer` or keep this as a thin wrapper delegating to shared utilities.

---

### Low
- Unused parameter in `record_error`
  - `_error_type` is ignored; classification is recomputed from `http_status`.
  - Evidence:
```46:61:src/core/network/error_tracker.rs
pub fn record_error(&mut self, http_status: u16, _error_type: String, message: String) { /* ... */ }
```
  - Recommendation: Remove the parameter or use it as a hint when `http_status == 0`.

- Uses `eprintln!` for warnings instead of sidecar logger
  - Spec recommends a `DebugLogger` sidecar. Using the logger would keep diagnostics in `~/.claude/ccstatus/ccstatus-debug.log`.
  - Evidence: multiple `eprintln!` calls (e.g., oversized/malformed JSON warnings).

- Percentile utility duplicative
  - `calculate_percentiles()` duplicates functionality expected around `HttpMonitor` rolling stats. Keeping one canonical implementation avoids drift.

---

### Meets Requirements
- Memory-only; does not write state files.
- Provides error classification aligned with spec mappings for HTTP codes, including 429, 504, 529, and 0 → `connection_error`.
- Offers helpful diagnostics APIs (latest error, stats, cleanup), usable for development observability.
- JSONL timestamp parsing supports ISO-8601 with timezone and UTC fallback.

---

### Recommendations (Ordered)
1. Enforce that RED gating uses `JsonlMonitor::scan_tail()` only; annotate `has_recent_errors()` as diagnostics-only.
2. Change `network_error` fallback to `unknown_error` in `classify_connection_error`.
3. Remove or consolidate `determine_status()` and `calculate_percentiles()` into shared utilities used by `HttpMonitor`/`StatusRenderer`.
4. Drop unused `_error_type` parameter or use as a hint when `http_status == 0`.
5. Replace `eprintln!` with `DebugLogger` when debug is enabled.
