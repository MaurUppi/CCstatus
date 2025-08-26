### CCStatus Network Monitoring — End-to-End Test Proposal (COLD/GREEN/RED)

- **Goal**: Verify the full pipeline works as designed: stdin → orchestration (COLD/RED/GREEN) → probe → state persistence → rendering, with correct frequency gating, classification, and debug observability.
- **Spec basis**: `project/network/1-network_monitoring-Requirement-Final.md` (COLD/GREEN/RED sequence diagrams, stdin JSON, error mapping, debug logger behavior).
- **Real transcript anchor**: `transcript_path="/Users/ouzy/.claude/projects/-Users-ouzy-Documents-DevProjects-CCstatus/2129c2b8-0f7a-43d6-866c-ea87e2862e26.jsonl"` (sessionId: `6ed29d2f-35f2-4cab-9aae-d72eb7907a78`).

---

### 1) Test fixtures and setup

- **Artifacts/paths**
  - State file (single writer): `~/.claude/ccstatus/ccstatus-monitoring.json`
  - Debug log (sidecar): `~/.claude/ccstatus/ccstatus-debug.log`
  - Transcript (real): `/Users/ouzy/.claude/projects/-Users-ouzy-Documents-DevProjects-CCstatus/2129c2b8-0f7a-43d6-866c-ea87e2862e26.jsonl`
- **Env toggles**
  - `ANTHROPIC_BASE_URL`, `ANTHROPIC_AUTH_TOKEN` (env > shell > config)
  - `CCSTATUS_DEBUG=true` to assert sidecar lines
  - Optional `ccstatus_TIMEOUT_MS` to verify override (min(env, 6000))
- **Reset**
  - Remove the state file before scenarios that depend on cold-start: `rm -f ~/.claude/ccstatus/ccstatus-monitoring.json`
  - Remove debug log to isolate each scenario: `rm -f ~/.claude/ccstatus/ccstatus-debug.log`
- **stdin payload template** (fill dynamic fields per case):
```json
{
  "session_id": "6ed29d2f-35f2-4cab-9aae-d72eb7907a78",
  "transcript_path": "/Users/ouzy/.claude/projects/-Users-ouzy-Documents-DevProjects-CCstatus/2129c2b8-0f7a-43d6-866c-ea87e2862e26.jsonl",
  "cwd": "/Users/ouzy/Documents/DevProjects/CCstatus",
  "model": {"id": "claude-sonnet-4-20250514", "display_name": "Sonnet 4"},
  "workspace": {"current_dir": "/Users/ouzy/Documents/DevProjects/CCstatus", "project_dir": "/Users/ouzy/Documents/DevProjects/CCstatus"},
  "version": "1.0.89",
  "output_style": {"name": "default"},
  "cost": {
    "total_cost_usd": 0.0,
    "total_duration_ms": 602500,
    "total_api_duration_ms": 0,
    "total_lines_added": 0,
    "total_lines_removed": 0
  },
  "exceeds_200k_tokens": false
}
```
- **Window cheat sheet**
  - GREEN window: `(total_duration_ms % 300_000) < 3_000` → e.g., `602_500` (hit), `605_000` (skip)
  - RED window: `(total_duration_ms % 10_000) < 1_000` → e.g., `100_500` (hit), `101_200` (skip)
  - COLD: prioritized when no valid state or new `session_id` not previously used for COLD (see de-dupe rules)

---

### 2) Expected classifications (from spec)

- 200–299 → `success`
- 0 → `connection_error` (timeout/connection failure)
- 400 → `invalid_request_error`
- 401 → `authentication_error`
- 403 → `permission_error`
- 404 → `not_found_error`
- 413 → `request_too_large`
- 429 → `rate_limit_error` (overall status becomes degraded)
- 500 → `api_error`
- 504 → `socket_hang_up`
- 529 → `overloaded_error`
- Other 4xx → `client_error`; other 5xx → `server_error`; else → `unknown_error`

---

### 3) Test matrix (end-to-end)

Each test drives the binary via stdin once (no background threads), then verifies state file content and, when enabled, debug log lines.

- CREDENTIALS
  - 3.1 No credentials
    - Setup: unset `ANTHROPIC_*`; state missing
    - Input: any window
    - Expect: `status=unknown`, `monitoring_enabled=false`, `api_config.source=null|omitted`, no probe sent; renderer ⚪ Unknown; debug shows creds source: none
  - 3.2 Env credentials
    - Setup: set `ANTHROPIC_BASE_URL`, `ANTHROPIC_AUTH_TOKEN`
    - Expect: creds source=`environment` in `api_config.source`

- COLD (single writer semantics; prioritized; de-dup by `session_id` and state markers)
  - 3.3 Cold start → 200
    - Setup: state missing; valid creds; any window
    - Expect: one probe with GREEN-like write rules, `rolling_totals` appended, `p95_latency_ms` computed, `status` healthy/degraded by P80/P95, `monitoring_state.last_cold_session_id` and `last_cold_probe_at` set (local time)
  - 3.4 Cold start → 429/5xx/timeout
    - Expect: update instantaneous fields only, no `rolling_totals/p95`, `status=degraded` for 429 else `error`
  - 3.5 Cold de-dup in same session
    - Setup: reuse same `session_id` in subsequent stdin
    - Expect: no second COLD; priority falls through to RED/GREEN checks

- GREEN window (frequency gate; only adds to `rolling_totals` when 200)
  - 3.6 GREEN → 200
    - Input: `total_duration_ms=602_500` (hits GREEN)
    - Expect: update `latency_ms/breakdown/last_http_status/error_type/status`; append to `rolling_totals` (cap=12), recompute `p95_latency_ms`; status healthy/degraded by P80/P95
  - 3.7 GREEN → 429
    - Expect: degraded; no `rolling_totals/p95` update
  - 3.8 GREEN → 5xx/timeout
    - Expect: error; no `rolling_totals/p95` update
  - 3.9 GREEN window skip
    - Input: value that misses GREEN
    - Expect: no state write; renderer uses last state

- RED window (error-driven; requires transcript error + window hit; never updates rolling/P95)
  - 3.10 RED gate requires JSONL error
    - Pre-step: append error row to transcript (see helper below) with `isApiErrorMessage=true`
    - Input: `total_duration_ms=100_500` (hits RED)
    - Expect: probe executed with RED timeout; write instantaneous fields + `last_jsonl_error_event`; `status=error`; no `rolling_totals/p95`
  - 3.11 RED gate skipped when window misses
    - Input: `total_duration_ms=101_200` (misses RED) with error present
    - Expect: skip probe; last state retained

- PRIORITY AND SINGLE-PROBE GUARANTEE
  - 3.12 Priority order COLD > RED > GREEN
    - Setup: craft input that could hit both RED and GREEN; also ensure transcript has error
    - Expect: exactly one probe executed according to priority; debug shows single probe lifecycle

- ROLLING P95
  - 3.13 Rolling cap and percentile
    - Drive ≥12 successful GREEN/COLD 200 samples; verify `rolling_totals` length capped at 12 and `p95_latency_ms` matches percentile over the last 12

- TIMEOUT OVERRIDE
  - 3.14 `ccstatus_TIMEOUT_MS`
    - Setup: `ccstatus_TIMEOUT_MS=1800`
    - Expect: both RED/GREEN use `min(1800, 6000)=1800` timeout; debug shows timeout setting

- ATOMICITY AND TIMESTAMPS
  - 3.15 Atomic write
    - Indirectly assert: state file is never partially written across runs; e.g., run with parallel invocations blocked at process level (if harness available) or check file integrity after abrupt termination tests
  - 3.16 Local time formatting
    - Expect: all persisted timestamps (including `last_cold_probe_at`, `timestamp`) are local ISO‑8601 with offset (e.g., `+08:00`)

- RENDERER INTEGRATION (smoke)
  - 3.17 StatusRenderer mapping
    - For statuses healthy/degraded/error/unknown, verify emoji + summary mapping and P95/breakdown presence per spec

---

### 4) Debug verification (CCSTATUS_DEBUG=true)

- Expect sidecar log lines per module:
  - NetworkSegment: stdin received; windows computed (COLD/RED/GREEN)
  - CredentialManager: creds source and token length
  - JsonlMonitor: error_detected?, code/message/timestamp
  - HttpMonitor: probe start/stop; timeout; http_status; breakdown; state write summary (status, p95, rolling_len)
  - StatusRenderer: render emoji/status summary

Minimal Rust helper to assert debug output and simplify transcript error injection for RED tests:

```rust
use std::{fs, io::Write, path::Path, time::Duration};

const DEBUG_LOG: &str = "${HOME}/.claude/ccstatus/ccstatus-debug.log"; // expand in your harness

pub fn read_debug_log() -> String {
    fs::read_to_string(shellexpand::tilde(DEBUG_LOG).to_string()).unwrap_or_default()
}

pub fn assert_debug_contains(needle: &str) {
    let log = read_debug_log();
    assert!(log.contains(needle), "debug log did not contain: {}", needle);
}

pub fn append_jsonl_error_event(transcript_path: &str) {
    // Append a minimal error record to trigger RED gate
    let err = r#"{
      \"type\": \"assistant\",
      \"timestamp\": \"2025-08-26T01:23:45.000Z\",
      \"message\": { \"content\": [{ \"type\": \"text\", \"text\": \"API Error: 529 {\\\"type\\\":\\\"error\\\",\\\"error\\\":{\\\"type\\\":\\\"overloaded_error\\\",\\\"message\\\":\\\"Overloaded\\\"}}\" }] },
      \"isApiErrorMessage\": true
    }\n"#;
    let path = Path::new(transcript_path);
    let mut f = fs::OpenOptions::new().create(true).append(true).open(path).unwrap();
    f.write_all(err.as_bytes()).unwrap();
}
```

Note: If your tests already provide a transcript mutator, prefer that over this helper.

---

### 5) Sample scenario scripts (bash harness-friendly)

- GREEN hit example:
```bash
export CCSTATUS_DEBUG=true
export ANTHROPIC_BASE_URL="https://api.anthropic.com"
export ANTHROPIC_AUTH_TOKEN="<redacted>"
jq '.cost.total_duration_ms=602500' stdin.template.json | your_binary_under_test
```

- RED hit example (after injecting error line):
```bash
export CCSTATUS_DEBUG=true
append_jsonl_error_event \
  "/Users/ouzy/.claude/projects/-Users-ouzy-Documents-DevProjects-CCstatus/2129c2b8-0f7a-43d6-866c-ea87e2862e26.jsonl"
jq '.cost.total_duration_ms=100500' stdin.template.json | your_binary_under_test
```

- Unknown credentials:
```bash
unset ANTHROPIC_BASE_URL ANTHROPIC_AUTH_TOKEN
jq '.cost.total_duration_ms=602500' stdin.template.json | your_binary_under_test
```

---

### 6) Acceptance checks per scenario

For each run, assert on `~/.claude/ccstatus/ccstatus-monitoring.json`:
- `status` in { healthy | degraded | error | unknown }
- `monitoring_enabled` boolean is correct for creds presence
- `api_config.endpoint/source` matches source and target URL
- `network.latency_ms`, `breakdown`, `last_http_status`, `error_type` align with response
- `rolling_totals` length cap=12 and updated only on 200 in GREEN/COLD
- `p95_latency_ms` present and correct when enough samples exist
- RED never updates `rolling_totals/p95`, but writes `last_jsonl_error_event`
- All timestamps are local ISO‑8601 with offset
- Debug log contains per-module breadcrumbs for the path taken

---

### 7) Coverage to spec mapping

- COLD/GREEN/RED sequence and ordering: 3.3–3.12
- CredentialManager behavior: 3.1–3.2
- HttpMonitor persistence and single-writer rule: all state assertions
- JsonlMonitor gate for RED: 3.10–3.11
- Frequency gates and de-dupe: window cheat sheet, 3.5–3.12
- Rolling P95 policy: 3.6–3.9, 3.13
- Timeout policy and override: 3.14
- DebugLogger sidecar: section 4
- Rendering mapping: 3.17

This proposal is grounded on the requirement doc and the real JSONL transcript path/session, and is ready to execute against the implemented modules and tests. 