# Claude Local Development Guide (Rust + Tokio + isahc)

This project is stdin-triggered (no background threads). Tokio provides the async runtime; isahc is the HTTP client. HttpMonitor is the single writer for monitoring state.

## Runtime & HTTP
- Runtime: Tokio (multi-threaded). Do not spawn tasks that outlive a single stdin event.
- HTTP: isahc
  - Reuse a single `HttpClient` per process invocation.
  - Configure per-request timeout with `Configurable::timeout`.
  - TTFB/Total measured via local timers; if DNS/TCP/TLS metrics are unavailable, set them to 0 and keep TTFB/Total accurate.
  - Optional: enable isahc metrics feature to collect handshake breakdowns when available.

## Pre‑action checklist (recall before each change)
- Trigger model: stdin only; no background loops/timers
- Single writer: only `HttpMonitor` writes state
- Probe budget: at most one probe per stdin (COLD > RED > GREEN)
- Credentials: resolve (env > shell > config); if none, `write_unknown(false)` and exit
- RED gating: `JsonlMonitor::scan_tail()` each stdin (non‑COLD) + 10s/1s window
- Timeouts: GREEN `clamp(p95+500, 2500, 4000)` or 3500 if samples < 4; RED 2000; `min(CCSTATUS_TIMEOUT_MS, 6000)`
- Rolling stats: update only on GREEN 200; cap 12; recompute P95
- Timestamps: persist local ISO‑8601 with offset; convert transcript `Z` to local for `last_jsonl_error_event`
- COLD dedup: persist real `session_id` + `last_cold_probe_at`
- Logging: reuse one correlation_id for probe start/end; no secrets; append‑only JSONL

## Module Boundaries
- CredentialManager: resolve creds (env > shell > config). No state writes.
- JsonlMonitor: scan transcript tail; return `(error_detected, last_error_event)`. Memory-only.
- HttpMonitor: executes probe and atomically writes `~/.claude/ccstatus/ccstatus-monitoring.json`. Only writer.
- StatusRenderer: pure formatting; single-line status.
- DebugLogger: sidecar logs gated by `CCSTATUS_DEBUG`; never influences control flow.

## HttpMonitor API (integration)
- `new(state_path: Option<PathBuf>) -> Self` (pass explicit path in HOME-less envs)
- `probe(mode: ProbeMode, creds: ApiCredentials, last_jsonl_error_event: Option<JsonlError>) -> Result<ProbeOutcome, NetworkError>`
- `write_unknown(monitoring_enabled: bool) -> Result<(), NetworkError>`
- `load_state() -> Result<MonitoringSnapshot, NetworkError>` (read-only)

ProbeMode and behavior
- Modes: `COLD`, `GREEN`, `RED` (priority: COLD > RED > GREEN; at most one probe per stdin)
- Timeouts: GREEN = `clamp(p95+500, 2500, 4000)` or 3500 if samples < 4; RED = 2000; both overridden by `CCSTATUS_TIMEOUT_MS` using `min(env, 6000)` (for compatibility, `ccstatus_TIMEOUT_MS` may also be honored until unified)
- Rolling stats: append only GREEN 200 samples; cap 12; recompute P95
- RED path: never update `rolling_totals/p95`; write `last_jsonl_error_event`
- Errors: map to standard `error_type`; network failures use `last_http_status=0` and `connection_error`
- Writes: temp + rename; timestamps in local ISO-8601 with offset

## NetworkSegment Orchestration
- Inputs: `total_duration_ms`, `transcript_path`, `session_id`
- Steps: get creds → write_unknown if none → scan Jsonl tail → choose window (COLD dedup by `last_cold_session_id`) → probe once → render from state
- COLD dedup persistence: pass the real `session_id` to `HttpMonitor` on COLD so it writes `monitoring_state.last_cold_session_id` and `last_cold_probe_at` (local time).

## RED gating (source of truth)
- Detection: `JsonlMonitor::scan_tail(transcript_path)` each stdin (non‑COLD)
- Window: trigger RED only when `(total_duration_ms % 10_000) < 1_000`
- Diagnostics‑only: `ErrorTracker::has_recent_errors()` must not drive RED

## State File Invariants
- Only HttpMonitor writes
- Schema: `status`, `monitoring_enabled`, `api_config.endpoint/source`, `network.{latency_ms, breakdown, last_http_status, error_type, rolling_totals[], p95_latency_ms}`, `monitoring_state.{last_green_window_id, last_red_window_id, last_cold_session_id, last_cold_probe_at}`, `last_jsonl_error_event`, `timestamp`
- All persisted timestamps are local ISO-8601 with offset
- `api_config.source` uses stable lowercase values (e.g., `environment`, `shell`, `claude_config`)

## Logging (DebugLogger)
- `CCSTATUS_DEBUG` is a flexible boolean: true/1/yes/on (case-insensitive)
- Output: `~/.claude/ccstatus/ccstatus-debug.log` (append-only). No secrets.
- Log: stdin/windows; creds source + token length; RED detection summary; probe start/stop, timeout, status, breakdown; state write summary; render summary
- Correlation: reuse a single `correlation_id` for `probe_start` and `probe_end`; include `breakdown` on `probe_end` when available

## isahc Usage Sketch
```rust
use isahc::{HttpClient, Request, ReadResponseExt};
use isahc::config::Configurable;

let client = HttpClient::new()?;
let url = format!("{}/v1/messages", base_url);
let body = serde_json::to_vec(&payload)?;
let req = Request::post(url)
    .header("Content-Type", "application/json")
    .header("x-api-key", auth_token)
    .body(body)?
    .timeout(timeout);
let start = std::time::Instant::now();
let mut resp = client.send_async(req).await?;
let ttfb_ms = start.elapsed().as_millis() as u32;
let status = resp.status().as_u16();
let _ = resp.text().unwrap_or_default();
```

## Testing rules
- No `#[cfg(test)]` in production files; put integration tests under `tests/` mirroring the source tree.
  - Mapping: `src/{path}/{module}.rs` → `tests/{path}/{module}_tests.rs`
- Use `#[tokio::test]` for async.
- Use `tokio::time::pause`/`advance` to avoid real delays.
- Integration tests: window gating (COLD/RED/GREEN), single-writer invariants, timeout rules, error typing.
- Inject fakes for HTTP and clock (traits/DI). Avoid in-file `#[cfg(test)]`; expose minimal `pub(crate)` as needed.

### Example
```rust
// tests/credential_test.rs
use tokio::time::{self, Duration};

#[tokio::test]
async fn resolves_env_credentials() {
    time::pause();
    // arrange
    // act
    // time::advance(Duration::from_millis(100)).await;
    // assert
}
```

## Performance & Safety
- Avoid blocking in async paths; offload file I/O to blocking thread when needed
- No background loops or timers; everything is stdin-triggered
- Reuse allocations and clients within a single invocation

## Environment
- `ANTHROPIC_BASE_URL`, `ANTHROPIC_AUTH_TOKEN`
- `CCSTATUS_TIMEOUT_MS` (min with 6000). Note: for compatibility, some paths may also accept `ccstatus_TIMEOUT_MS` until unified
- `CCSTATUS_DEBUG` (flexible boolean)
- Optional: `CCSTATUS_JSONL_TAIL_KB` for transcript tail size

## StatusRenderer
- Default to single-line output for statusline; if line wrapping is enabled, only the first line appears in the status bar.
