### DebugLogger — Review against Network Monitoring Pipeline v2

Date: 2025-08-25
Target file: `src/core/network/debug_logger.rs`

---

#### Classification Overview
- Critical: 1 finding
- Medium: 3 findings
- Low: 2 findings

---

### Critical
- Log truncation on process start wipes history
  - Behavior: the log file is cleared once per process lifetime; the statusline runs as short-lived processes, effectively erasing prior logs repeatedly.
  - Evidence:
```rust
// Session refresh - clear log file at start of each session
LOG_INIT.call_once(|| {
    if enabled {
        let _ = std::fs::write(&log_path, "");
    }
});
```
  - Impact: Unable to correlate multi-event flows; defeats purpose of persistent sidecar logging.
  - Requirement mismatch: Spec expects an append-only sidecar with persistent history; no truncation specified.

---

### Medium
- Environment variable precedence deviates from spec
  - Spec: `CCSTATUS_DEBUG` gates logging. Implementation prioritizes `ccstatus_debug` and treats empty `CCSTATUS_DEBUG` as enabled.
  - Risk: Surprise activations or suppression; inconsistent with documented envs.

- Logs are not structured (machine-parseable)
  - Current: bracketed free-form strings.
  - Spec intent: "结构化日志" suitable for downstream analysis. Recommend JSON Lines with stable keys: `timestamp`, `level`, `component`, `event`, `fields`.

- No default redaction guardrails on generic APIs
  - `debug/error/performance` accept arbitrary strings; callers may accidentally log secrets. Only `credential_info` is safe by design.
  - Recommend: typed helpers that accept fields and perform redaction; deny-list for headers like `authorization`, values longer than N, etc.

---

### Low
- Timestamps omit timezone offset
  - State file requires local ISO-8601 with offset; debug log not mandated but aligning helps correlation across artifacts.

- Missing typed helpers for key events (consistency)
  - Suggested helpers per spec "日志要点":
    - `probe_start(mode, timeout_ms)` / `probe_end(status, breakdown, http_status)`
    - `state_write_summary(status, p95, rolling_len)`
    - `jsonl_error_summary(code, message, timestamp)`
    - `render_summary(emoji, status)`

---

### Meets Requirements
- Off by default; enabled via env gating.
- Log path: `~/.claude/ccstatus/ccstatus-debug.log`; parents created.
- Sidecar behavior: async file writes via blocking thread; no control-flow decisions; errors ignored.
- `credential_info` avoids secret leakage (logs only source and token length).

---

### Recommendations (Ordered)
1. Remove log truncation; always append-only.
   - Acceptance: multiple statusline invocations append without loss.
2. Align gating with spec: honor `CCSTATUS_DEBUG` exclusively (or make lowercase alias secondary and documented). Treat empty value as `false`.
   - Acceptance: `CCSTATUS_DEBUG=true|1|on|yes` enables; others disable.
3. Emit JSONL records with fixed schema and local-time ISO-8601 timestamps with offset.
   - Example record:
```json
{"ts":"2025-08-25T15:31:02.123+08:00","level":"DEBUG","component":"HttpMonitor","event":"probe_start","fields":{"mode":"GREEN","timeout_ms":3500}}
```
4. Provide typed helpers with built-in redaction for "日志要点" events; restrict generic `debug/error` for internal use.
5. Add simple redaction utility (mask long strings, scrub known secret keys) and unit tests covering redaction and JSONL shape.

---

### Risk/Impact
- Addressing the Critical issue restores debuggability across sessions.
- Structured JSONL enables downstream grep/jq tooling and future automated analysis.
- Redaction reduces accidental sensitive-data leakage.
