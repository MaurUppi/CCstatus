### StatusRenderer — Review against Network Monitoring Pipeline v2

Date: 2025-08-25
Target file: `src/core/network/status_renderer.rs`

---

#### Classification Overview
- Critical: 0 findings
- Medium: 3 findings
- Low: 3 findings

---

### Medium
- Missing `error_type` display for degraded/error
  - Spec: degraded/error should display breakdown and `error_type`.
  - Current: degraded shows P95 + breakdown; error shows breakdown only; neither includes `error_type`.
```26:39:src/core/network/status_renderer.rs
NetworkStatus::Degraded => {
    // degraded: show P95 and breakdown (wrap if long)
    let p95_display = if metrics.p95_latency_ms == 0 {
        "P95:N/A".to_string()
    } else {
        format!("P95:{}ms", metrics.p95_latency_ms)
    };
    let base = format!("🟡 {}", p95_display);
    self.format_with_breakdown(base, &metrics.breakdown)
},
NetworkStatus::Error => {
    // error: show breakdown (wrap if long)
    self.format_with_breakdown("🔴".to_string(), &metrics.breakdown)
},
```

- Multi-line wrapping conflicts with statusline behavior
  - Spec (statusline): the first line of stdout becomes the status line text. Adding a newline means the wrapped details won’t show in the UI.
  - Current: inserts a `\n` when length exceeds 80.
```46:56:src/core/network/status_renderer.rs
if base.len() + breakdown.len() + 1 > max_line_length {
    format!("{}\n{}", base, breakdown)
} else {
    format!("{} {}", base, breakdown)
}
```
  - Recommendation: keep single-line output; truncate with ellipsis or shorten fields instead of newline.

- Unknown status text misleads about cause
  - Spec: ⚪ unknown = no credentials or no recent results. Current text explicitly blames env vars and is not localized.
```40:43:src/core/network/status_renderer.rs
NetworkStatus::Unknown => {
    "⚪ Env varis NOT Found".to_string()
}
```
  - Recommendation: neutral message (e.g., "⚪ Unknown") or accept an external reason string from the caller.

---

### Low
- Typo/grammar in unknown text
  - "Env varis NOT Found" → "Env vars not found" (and still better to avoid blaming envs).

- Width calculation uses byte length
  - `.len()` counts bytes, not display width; emoji and wide glyphs distort the 80-char heuristic. If keeping width logic, consider a simple grapheme/ANSI-aware length or fixed truncation.

- No debug sidecar log for render summary
  - Spec suggests `DebugLogger` should log a final render summary (emoji/status/thresholds). This can be done by the caller, but adding an optional hook would improve consistency.

---

### Meets Requirements
- Emoji mapping: 🟢/🟡/🔴/⚪ align with healthy/degraded/error/unknown.
- Healthy/degraded display P95 value.
- Degraded/error display the latency breakdown (partially compliant; missing `error_type`).
- Pure renderer: no state writes, no I/O.

---

### Recommendations (Ordered)
1. Append `error_type` when present for degraded and error, e.g., `... | err=rate_limit_error`.
2. Keep output single-line; truncate or compress breakdown to fit (e.g., cap to ~80 visible chars with ellipsis).
3. Replace unknown text with a neutral, accurate message ("⚪ Unknown"). Optionally accept a reason string.
4. Optionally emit a render-summary entry via `DebugLogger` when enabled (done by caller or via a helper).
