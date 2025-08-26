### DebugLogger — Review against Network Monitoring Pipeline v2

Date: 2025-08-25
Target file: `src/core/network/debug_logger.rs`

---

#### Classification Overview
- Critical: 0 findings
- Medium: 0 findings
- Low: 6 findings

---

### Critical
- None. Previous truncation behavior is removed; logging is append-only with size-based rotation.

---

### Low
- Use error message in `jsonl_error_summary`
  - Evidence:
```337:345:src/core/network/debug_logger.rs
pub fn jsonl_error_summary(&self, error_code: &str, _error_message: &str, timestamp: &str) {
    /* logs code and timestamp; ignores message */
}
```
  - Recommendation: include the parsed error message in `message` and/or `fields`.

- Hardcoded rotation settings (optional configurability)
  - Evidence:
```16:20:src/core/network/debug_logger.rs
const LOG_ROTATION_SIZE_MB: u64 = 8;
const MAX_ARCHIVES: u32 = 5;
const ROTATION_CHECK_INTERVAL: u32 = 200;
```
  - Recommendation: allow env overrides (e.g., `CCSTATUS_DEBUG_ROTATE_MB`, `CCSTATUS_DEBUG_MAX_ARCHIVES`).

- No redaction on structured fields
  - Message text is redacted; `fields` are written verbatim.
  - Recommendation: either apply redaction to string-like field values or document that typed helpers must be used for any sensitive data.

- Backward-compat alias missing
  - Tests/imports may still refer to `DebugLogger` while code exports `EnhancedDebugLogger`.
  - Recommendation: add `pub type DebugLogger = EnhancedDebugLogger;` for compatibility, or re-export under that name.

- Rotation lock file cleanup on error
  - `try_lock_exclusive` uses a `.lock` file and removes it on success path.
  - Recommendation: best-effort cleanup on early-return error paths to avoid stale lock files (low impact).

- Minor test/docs alignment
  - Performance log now stores duration in `fields.duration_ms`; some older docs/tests expect "X took Yms" in message. Consider clarifying docs or adjusting message for human readability.

---

### Meets Requirements
- Off by default; enabled via `CCSTATUS_DEBUG` with flexible booleans (true/1/yes/on).
- Path: `~/.claude/ccstatus/ccstatus-debug.log`; parents created; falls back to current dir if HOME missing.
- Append-only JSON Lines with fixed schema and local-time ISO-8601 timestamps with offset.
- Size-based rotation with gzip archives and bounded retention; basic inter-process rotation lock.
- Redaction guardrails on message content; typed helpers for key events (`probe_start/end`, `state_write_summary`, `jsonl_error_summary`, `render_summary`, `credential_info_safe`).
- No influence on control flow; logging failures are non-fatal.

---

### Recommendations (Ordered)
1. Include error message in `jsonl_error_summary` output for richer diagnostics.
2. Allow optional env overrides for rotation size/archives/check interval.
3. Apply redaction to string-like `fields` or document safe usage of typed helpers.
4. Add a compatibility alias `pub type DebugLogger = EnhancedDebugLogger;` (or re-export) to avoid breaking callers.
5. Add best-effort lock file cleanup in rotation error paths.
6. Align docs/tests with JSONL schema and current message formatting.

---

### Risk/Impact
- Changes since last review significantly improve compliance and observability: persistent JSONL, rotation, redaction, typed helpers.
- Remaining items are niceties and polish; low operational risk.
