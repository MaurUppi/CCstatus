### HttpMonitor — Review against Network Monitoring Pipeline v2

Date: 2025-08-26
Target file: `src/core/network/http_monitor.rs`

---

#### Classification Overview
- Critical: 0 findings
- High/Medium: 1 finding
- Low: 2 findings

---

### Resolved (previous findings now fixed)
- COLD dedup writes real `session_id` and local `last_cold_probe_at`
  - Implemented via `HttpMonitor::set_session_id` and applied in COLD branch.
  - Code: COLD path persists `last_cold_session_id` and `last_cold_probe_at` when `current_session_id` is set.
- RED `last_jsonl_error_event.timestamp` converted to local time
  - Code converts UTC `'Z'` timestamps using `convert_utc_to_local_timestamp` before persisting.
- Timeout env var compatibility
  - Code accepts both `CCSTATUS_TIMEOUT_MS` and `ccstatus_TIMEOUT_MS` (
    `get_timeout_env_var()`), capped at 6000ms.
- Probe start/end correlation
  - A single `probe_id` is generated and reused for start/end logs.
- Stable `api_config.source`
  - `CredentialSource` implements `Display` returning stable lowercase values; used when persisting `api_config.source`.

---

### High/Medium
- Success gating limited to 200 only (should be 200–299)
  - Current: GREEN/COLD add to `rolling_totals` only when `metrics.last_http_status == 200`.
  - Spec: 200–299 are success; broaden check to the entire 2xx range.
  - Suggested edit:
    - In GREEN/COLD branch, change condition to: `if (200..=299).contains(&metrics.last_http_status) { ... }`.

### Low
- `write_unknown(monitoring_enabled: bool)` parameter can mislead callers
  - Spec usage always sets `monitoring_enabled=false`. Consider hard-coding to `false` internally or deprecating the param to avoid misuse.
- Observability polish
  - Optionally include `breakdown` in `network_probe_end` or an additional debug field to ease troubleshooting.

---

### Meets Requirements
- Single writer: Only `HttpMonitor` writes `~/.claude/ccstatus/ccstatus-monitoring.json` with atomic temp+rename.
- Probe modes: COLD/GREEN/RED behaviors implemented; GREEN adaptive timeout, RED fixed 2000ms.
- Timeout override: Env and test override honored, capped at 6000ms.
- Lightweight probe: POST `{base_url}/v1/messages`, header `x-api-key`, minimal payload.
- Error classification: Standardized per spec (0, 4xx, 5xx, 429, 504, 529, etc.).
- Rolling stats: Only GREEN/COLD success appends (cap=12) and recalculates P95; RED never updates rolling stats.
- Timestamps: Persisted in local timezone ISO‑8601 with offset.
- Observability: Probe start/end and state write summary logged via `DebugLogger`.

---

### Actionable resolutions
1) Update success gating to 2xx for GREEN/COLD appends and status computation.
2) Consider hard-wiring `monitoring_enabled=false` inside `write_unknown` (and/or rename method to clarify semantics).
3) Optional: Add `breakdown` to the probe-end log for richer diagnostics.

---


