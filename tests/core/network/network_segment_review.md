# NetworkSegment Review (stdin orchestration)

Date: 2025-08-26
Target: `src/core/network/network_segment.rs`
Spec: `project/network/1-network_monitoring-Requirement-Final.md` (v2, 2025-08-25)

## Requirement coverage
- Stdin-driven, no background threads: Met. Uses single async flow, no persistent tasks.
- Inputs parsed: Met. Parses `session_id`, `transcript_path`, `cost.total_duration_ms`.
- Credential resolution: Met. Calls `CredentialManager::get_credentials()` each run; no caching.
- No-credential path: Met in module logic. Calls `HttpMonitor::write_unknown(false)`, then renders and returns. Note: Not exercised in app because `NetworkSegment` is not yet wired into the main statusline pipeline (see Integration gap below).
- Windowing: Met.
  - COLD: `(total_duration_ms < COLD_WINDOW_MS)` with `ccstatus_COLD_WINDOW_MS` override (also accepts `CCSTATUS_COLD_WINDOW_MS`).
  - RED: `(total_duration_ms % 10_000) < 1_000` gated by JSONL error detection.
  - GREEN: `(total_duration_ms % 300_000) < 3_000`.
  - Priority: COLD > RED > GREEN. At most one `probe()` per stdin event.
- RED gating via transcript: Partially met. RED decision consults `JsonlMonitor::scan_tail`, and RED probe passes `last_error_event` (see note below on double-scan).
- COLD dedup: Partially met. Dedup by `monitoring_state.last_cold_session_id`; `HttpMonitor` persists fields.
- Single writer: Met. Only `HttpMonitor` writes state; Segment only reads state to render.
- Render behavior: Met. When no window triggers, rendering uses last persisted state.
- Debug logging: Largely met. Logs stdin parse and window decision; relies on other modules for detailed probe/debug logs.

## Findings

### Critical
- None found blocking functional correctness relative to the v2 spec.

### Medium
- GREEN/RED per-window dedup missing
  - Impact: Within the first 3s (GREEN) or 1s (RED) windows, multiple stdin events can trigger multiple probes, increasing traffic beyond intent. Spec’s schema exposes `monitoring_state.last_green_window_id/last_red_window_id` presumably for this; neither Segment nor HttpMonitor updates/uses them.
  - Suggestion: In Segment, compute window IDs (e.g., `total_duration_ms / 300000` and `/ 10000`) and skip probe if equals last persisted ID. Persist via `HttpMonitor` to keep single-writer invariant.

- RED JSONL double-scan
  - Behavior: `calculate_window_decision()` scans tail to decide RED, and `run_from_stdin()` scans again to fetch `last_error_event` for the actual RED probe.
  - Impact: Duplicate I/O and duplicate debug logs during RED windows. Minor performance overhead but can be noticeable with frequent errors.
  - Suggestion: Move a single `(error_detected, last_error_event)` scan prior to window decision for non-COLD path, pass `error_detected` into RED gate decision and reuse `last_error_event` for `probe(RED, ...)`.

- COLD trigger semantics deviate from “cold_start” flow in spec
  - Behavior: Segment triggers COLD purely by time window `total_duration_ms < COLD_WINDOW_MS`, regardless of whether a valid state already exists.
  - Spec nuance: Mermaid flows describe COLD as contingent on “no valid state” (state missing or `status=unknown`). Earlier text separately defines a time-based COLD window.
  - Impact: May execute a redundant COLD probe on early events even when a valid state exists from a previous run/session.
  - Suggestion: Gate COLD by both conditions: (a) time window AND (b) current state invalid (file missing or `Unknown`). Keep session-id dedup.

### Low
- Debug log completeness
  - Segment logs “window decision” but not explicit “stdin received” and “windows computed (COLD/RED/GREEN)” phrasing called out in DebugLogger section. Non-blocking.
  - Suggestion: Add a single summary log after window calc including computed window IDs, aiding future per-window dedup.

- Env var naming
  - Segment accepts both `ccstatus_COLD_WINDOW_MS` and `CCSTATUS_COLD_WINDOW_MS`; the spec names the former. Keeping both is fine; consider documenting dual support in module docs for clarity.

## Overall assessment
- Functional parity within the module is high. The primary E2E blocker is integration: `NetworkSegment` is not currently executed by the app. Address that wiring first; then implement per-window dedup, single JSONL scan, and refined COLD semantics for optimal behavior.

## Suggested acceptance criteria adjustments
- Add per-window dedup to ensure ≤1 probe per window
- Single JSONL tail scan per stdin event (non-COLD path)
- COLD: require invalid state in addition to early time window
