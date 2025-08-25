### JsonlMonitor — Review against Network Monitoring Pipeline v2

Date: 2025-08-25
Target file: `src/core/network/jsonl_monitor.rs`

---

#### Classification Overview
- Critical: 0 findings
- Medium: 4 findings
- Low: 4 findings

---

### Medium
- Module boundary violation: persists "captured errors" to disk (debug mode)
  - Spec: "JsonlMonitor 自身不写状态"; only the `HttpMonitor` is the single writer. Debug persistence/logging should go through the `DebugLogger` sidecar.
  - Evidence:
```199:205:src/core/network/jsonl_monitor.rs
async fn parse_and_capture_errors(&mut self, content: &str) -> Result<(bool, Option<JsonlError>), NetworkError> {
    /* ... */
    if self.debug_mode {
        if self.capture_error(&error_entry).await? { /* ... */ }
    }
}
```
```377:402:src/core/network/jsonl_monitor.rs
async fn save_captured_errors(&self) -> Result<(), NetworkError> { /* writes ~/.claude/ccstatus/ccstatus-captured-error.json */ }
```

- Debug gating is too strict (`CCSTATUS_DEBUG` must equal "true")
  - Spec: boolean env; elsewhere we parse flexible booleans. Current logic enables debug only for exact string "true".
  - Evidence:
```41:47:src/core/network/jsonl_monitor.rs
let debug_mode = std::env::var("CCSTATUS_DEBUG").unwrap_or_default() == "true";
```
  - Recommendation: accept `true/1/yes/on` (case-insensitive) and `false/0/no/off`.

- `new()` fails when debug enabled but HOME/USERPROFILE missing
  - Impact: Enabling debug in restricted/container environments without home dir prevents creating `JsonlMonitor`, breaking RED detection.
  - Evidence:
```45:53:src/core/network/jsonl_monitor.rs
let home = std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE")).map_err(|_| NetworkError::HomeDirNotFound)?;
Some(PathBuf::from(home).join(".claude").join("ccstatus").join("ccstatus-captured-error.json"))
```
  - Recommendation: If home is unavailable, keep debug operational without persistence (in-memory only) instead of erroring.

- Error extraction assumes first content item; can miss valid errors
  - Only reads `message.content[0].text`. If an error appears in a later item, it is ignored.
  - Evidence:
```261:268:src/core/network/jsonl_monitor.rs
if let Some(first_content) = content_array.get(0) {
    if let Some(text) = first_content.get("text").and_then(|t| t.as_str()) {
        if let Some((code, _)) = self.parse_error_text(text) { /* ... */ }
    }
}
```
  - Recommendation: iterate all `content` items until a parsable error is found.

---

### Low
- Uses `eprintln!` for warnings instead of sidecar logger
  - Spec introduces `DebugLogger` as the logging sidecar; using it (when enabled) centralizes diagnostics.
  - Evidence: warnings for malformed/oversized JSON lines.

- Captured error schema uses string for boolean field
  - `is_api_error_message: "true"` is stored as string instead of boolean; not harmful but inconsistent.

- Flexible tail size env var is undocumented
  - `CCSTATUS_JSONL_TAIL_KB` is useful but not in the spec; consider documenting alongside other envs or default silently without new env.

- Timestamp conversion delegated to writer
  - Spec requires local-time ISO-8601 when persisting to state. JsonlMonitor returns raw transcript timestamps (UTC with Z). Ensure the writer (`HttpMonitor`) performs the conversion (note, not a defect if writer does this).

---

### Meets Requirements
- Reads only the transcript tail N KB per invocation; default 64KB; avoids full file scans.
- Detects `isApiErrorMessage: true` and returns `(error_detected, last_error_event)`; last error includes `code/message/timestamp`.
- Does not write the monitoring state file; RED gate control is returned to the caller for windowing.
- Robustness: skips malformed/oversized lines to avoid blocking error detection.

---

### Recommendations (Ordered)
1. Remove on-disk persistence from JsonlMonitor; route any debug details to `DebugLogger` (append-only JSONL) and keep JsonlMonitor memory-only.
2. Implement flexible boolean parsing for `CCSTATUS_DEBUG`; do not error on missing HOME—degrade to in-memory debug.
3. Iterate all `message.content` items to extract error code/message; keep the first parsed as the representative.
4. Replace `eprintln!` with optional sidecar logging when `DebugLogger` is enabled.
