### JsonlMonitor Enhancement Review (Fallback API Error Detection)

Date: 2025-08-27
Module: `src/core/network/jsonl_monitor.rs`
Reference proposal: `src/core/network/jsonl_monitor_enhancement_proposal.md`

---

### Overall verdict
Implemented as proposed. Primary flag path retained; fallback text-based detection added; schema unchanged.

---

### Critical
- None.

---

### Medium
- Unsafe UTF‑8 truncation in debug logging
  - Evidence: preview creation uses `&text[..50]`, which can split multi‑byte chars and panic.
  - Suggested fix:
```rust
let preview: String = text.chars().take(50).collect();
logger.debug_sync(
    "JsonlMonitor",
    "fallback_error_detected",
    &format!("API error detected via fallback path: {}", preview),
);
```

---

### Low
- Whitespace tolerance for “API Error”
  - Current: matches only `"api error"` or NBSP variant at start.
  - Improve: allow arbitrary whitespace (`^api\s*error`) to catch “API   Error …”.

- Code extraction requires a colon
  - Current: looks for `:` before scanning for a numeric code.
  - Improve: if no colon, also scan initial tokens for a 3‑digit code (e.g., “API error 429 …”).

- Optional anchoring
  - Current: anchored at start of text by design. Consider `contains` with guards if real transcripts show prefixed context.

---

### Acceptance criteria
- Fallback detection for unflagged “API error” lines: MET
- Schema unchanged (`JsonlError { timestamp, code, message }`): MET
- Robustness (malformed/oversized lines handled): MET

---

### Recommended minimal edits
- Replace `&text[..50]` with a safe `chars().take(50)` truncation.
- Optionally relax detection to `^api\s*error` and support numeric codes without a colon.
