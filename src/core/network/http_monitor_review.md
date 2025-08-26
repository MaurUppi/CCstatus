### HttpMonitor — Review against Network Monitoring Pipeline v2

Date: 2025-08-25
Target file: `src/core/network/http_monitor.rs`

---

#### Classification Overview
- Critical: 0 findings
- Medium: 3 findings
- Low: 4 findings

---

### Critical
- None.

---

### Medium
- COLD dedup fields persist placeholder session_id
  - Spec: On COLD probe, persist `monitoring_state.last_cold_session_id` to the real `session_id` and `last_cold_probe_at` (local time) for deduplication.
  - Evidence:
```561:566:src/core/network/http_monitor.rs
// COLD mode: Update session deduplication fields
if mode == ProbeMode::Cold {
    // TODO: Get actual session_id from NetworkSegment
    state.monitoring_state.last_cold_session_id = Some("placeholder".to_string());
    state.monitoring_state.last_cold_probe_at = Some(self.clock.local_timestamp());
}
```
  - Impact: Deduplication across sessions may be unreliable.
  - Recommendation: Accept `session_id` (or a context struct) in `probe(...)` or provide a dedicated setter invoked by `NetworkSegment` to persist the actual `session_id`.

- `last_jsonl_error_event.timestamp` not converted to local time before persistence
  - Spec: All timestamps persisted must be local-time ISO‑8601 with offset; transcript entries with `Z` must be converted.
  - Evidence:
```507:515:src/core/network/http_monitor.rs
ProbeMode::Red => {
    // RED mode: Set status=error, update error event, don't touch rolling stats
    if let Some(error_event) = last_jsonl_error_event {
        state.last_jsonl_error_event = Some(error_event);
    }
    state.status = NetworkStatus::Error;
    /* ... */
}
```
  - Impact: Mixed timezones in state file complicate correlation.
  - Recommendation: Convert `error_event.timestamp` to local time before writing.

- Timeout env var name mismatch with spec
  - Spec text references `ccstatus_TIMEOUT_MS` (lowercase prefix) while code reads `CCSTATUS_TIMEOUT_MS`.
  - Evidence:
```398:403:src/core/network/http_monitor.rs
if let Ok(env_timeout) = std::env::var("CCSTATUS_TIMEOUT_MS") {
    if let Ok(env_val) = env_timeout.parse::<u32>() {
        return Ok(std::cmp::min(env_val, 6000));
    }
}
```
  - Impact: Users following docs may set the lowercase variant and see no effect.
  - Recommendation: Accept both env names for compatibility.

---

### Low
- Probe start/end correlation IDs differ
  - Evidence:
```256:260:src/core/network/http_monitor.rs
debug_logger.network_probe_start(
    &format!("{:?}", mode),
    timeout_ms as u64,
    format!("probe_{}", uuid::Uuid::new_v4()),
);
```
```301:310:src/core/network/http_monitor.rs
debug_logger.network_probe_end(
    &format!("{:?}", mode),
    if status_code == 0 { None } else { Some(status_code) },
    latency_ms as u64,
    format!("probe_{}", uuid::Uuid::new_v4()),
);
```
  - Effect: Harder to correlate start/end of the same probe in logs.
  - Recommendation: Generate one ID and reuse for both calls.

- Unknown write method allows arbitrary `monitoring_enabled`
  - Spec usage always passes `false`. Current signature accepts any bool.
  - Evidence:
```342:370:src/core/network/http_monitor.rs
pub async fn write_unknown(&mut self, monitoring_enabled: bool) -> Result<(), NetworkError> { /* ... */ }
```
  - Recommendation: Either ignore the parameter and set `false`, or keep as-is but ensure callers always pass `false`.

- Debug logs omit breakdown on probe end
  - Spec suggests logging `breakdown` among observability fields.
  - Recommendation: Include `breakdown` in `network_probe_end` fields (optional polish).

- API config `source` relies on `Debug` text
  - Evidence:
```499:503:src/core/network/http_monitor.rs
state.api_config = Some(ApiConfig {
    endpoint: format!("{}/v1/messages", creds.base_url),
    source: format!("{:?}", creds.source).to_lowercase(),
});
```
  - Recommendation: Ensure `CredentialSource` `Debug` maps to stable lowercase values (e.g., `environment`, `shell`, `claude_config`), or implement `Display` explicitly.

---

### Meets Requirements
- Single writer: Only `HttpMonitor` writes `~/.claude/ccstatus/ccstatus-monitoring.json` with atomic temp+rename.
- Probe modes: COLD/GREEN/RED behaviors implemented; GREEN adaptive timeout, RED fixed 2000ms.
- Timeout override: Env and test override honored, capped at 6000ms.
- Lightweight probe: POST `{base_url}/v1/messages`, header `x-api-key`, minimal payload.
- Error classification: Maps status to standardized `error_type` per spec (429, 504, 529, etc.).
- Rolling stats: Only GREEN(200) appends to `rolling_totals` (cap=12) and recalculates P95; RED never updates rolling stats.
- State fields: Updates `status`, `network.{latency_ms, breakdown, last_http_status, error_type}`, `api_config.endpoint/source`, timestamp in local time.
- Observability: Logs probe start/end and state write summary via `DebugLogger`.

---

### Recommendations (Ordered)
1. Persist real `session_id` on COLD probes (extend `probe(...)` or add a setter invoked by orchestrator).
2. Convert `last_jsonl_error_event.timestamp` to local time before persisting.
3. Accept both `CCSTATUS_TIMEOUT_MS` and `ccstatus_TIMEOUT_MS` for compatibility.
4. Reuse a single correlation ID for probe start/end; optionally include `breakdown` on end.
5. Consider hard-coding `monitoring_enabled=false` in `write_unknown` to prevent misuse.
6. Use a stable `Display` for `CredentialSource` to generate `api_config.source`.
