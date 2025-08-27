### JsonlMonitor — Heuristic API Error Detection Enhancement

Goal: Capture API errors in transcript lines that do not set `isApiErrorMessage: true`, without changing persisted schemas or return types.


---
## Reference
- Error msg sample: `@project/API-error.png`

#### Current behavior
- `parse_jsonl_line` only returns an error when `isApiErrorMessage == true`.
- When an error is detected, `extract_transcript_error` parses `message.content[].text` using `parse_error_text` to derive `http_code` and `message`.
- `parse_error_text` currently matches only `"API Error: <code> ..."` with exact case and a leading prefix.

#### Problems observed
- Some errors (e.g., from screenshots/logs) show text like "API error ..." but do not set `isApiErrorMessage`.
- These lines are ignored by the monitor and do not trigger the RED gate.

---

#### Proposal (schema-preserving)
1. Add a secondary detection path in `parse_jsonl_line`:
   - If `isApiErrorMessage` is missing or false, scan `message.content[].text` items.
   - If any text item contains an API error signature that `parse_error_text` recognizes, treat the line as an error and return `Some(...)` via existing `extract_transcript_error`.

2. Improve `parse_error_text` to be case-insensitive and tolerant:
   - Recognize these patterns (case-insensitive):
     - `^API\s*Error\s*:\s*(?<code>\d{3})\b.*` → extract `code`
     - `^API\s*error\b.*` (no explicit code) → return `(0, "API Error")`
   - Continue supporting the existing JSON payload extraction (if a `{...}` JSON object is present after the prefix), falling back to friendly messages by known code.

3. Keep the public contract unchanged:
   - `scan_tail(...) -> (bool, Option<JsonlError>)` remains the same.
   - Persisted `JsonlError { timestamp, code, message }` remains unchanged.

---

#### Implementation sketch

- Edit `parse_jsonl_line`:
```rust
// After the isApiErrorMessage check returns None
if let Some(content_items) = json
    .get("message").and_then(|m| m.get("content")).and_then(|c| c.as_array())
{
    for item in content_items {
        if let Some(text) = item.get("text").and_then(|t| t.as_str()) {
            if self.parse_error_text(text).is_some() {
                // Reuse existing extractor to build TranscriptErrorEntry
                return Ok(Some(self.extract_transcript_error(&json)?));
            }
        }
    }
}
```

- Edit `parse_error_text`:
```rust
fn parse_error_text(&self, text: &str) -> Option<(u16, String)> {
    let lower = text.to_ascii_lowercase();
    if lower.starts_with("api error") || lower.starts_with("api\u{00a0}error") {
        // Try to extract numeric code after a colon, if present
        // e.g., "API Error: 429 ..." → code = 429
        if let Some(colon_idx) = lower.find(':') {
            let after = &lower[colon_idx + 1..];
            if let Some(code) = after.split_whitespace().find_map(|tok| tok.parse::<u16>().ok()) {
                // Preserve existing JSON parsing for detailed message if present
                // (current implementation already attempts to parse trailing JSON)
                // Fallback message by known codes as today
                // ...
                return Some((code, /* derive message as in current impl */ "API Error".to_string()));
            }
        }
        // No code present → treat as generic API error
        return Some((0, "API Error".to_string()));
    }
    None
}
```

Note: The existing code already attempts to parse a trailing JSON object (`{"error": {"message": "..."}}`). Keep that logic, just make the entry condition case-insensitive and colon/code optional.

---

#### Edge cases and safeguards
- Line length and malformed JSON handling remain unchanged (already robust and bounded).
- False positives are minimized by anchoring the phrase at the start of `text` ("API error..."). If needed, we can additionally require the item’s `type == "tool_result"` or similar, but not necessary initially.
- HTTP code absent → `code = 0`. Downstream logic already tolerates arbitrary codes.

---

#### Test plan (unit only; no I/O)
- When `isApiErrorMessage: true` and text is "API Error: 500 ..." → detected (no regression).
- When `isApiErrorMessage: false` and text is "API error: 429 ..." → detected with code 429.
- When `isApiErrorMessage: false` and text is "API error occurred" (no code) → detected with code 0 and message "API Error".
- Mixed content items: only one contains the signature → detected; `extract_transcript_error` still scans all items for the first code.
- Non-matching texts → not detected.

---

#### Rollout
- Backward compatible; no schema changes.
- Low-risk; limited to transcript parsing logic.
- Can be toggled later via an env flag if desired (not required for initial rollout).
