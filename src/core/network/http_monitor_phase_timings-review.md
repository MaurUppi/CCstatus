### HttpMonitor DNS/TCP/TLS phase timings review

Date: 2025-08-26
Module: `src/core/network/http_monitor.rs`
Plan: `project/network/4-dns_tcp_tls_phase_timings-dev-plan.md`

---

### Overall verdict
Substantial compliance. The curl timings path is now auto‑wired under `timings-curl` and `metrics.latency_ms` uses TTFB (preserving rolling semantics). Static‑curl feature is wired. Minor resiliency and test‑alignment improvements remain.

---

### Critical
None identified after the update.

---

### Medium
- No curl→isahc fallback on runner failure
  - Observation: If the curl runner errors, the probe returns a connection error instead of falling back to the isahc path.
  - Impact: Reduced resiliency; transient curl issues cause false negatives.
  - Fix (surgical): On curl error, invoke the isahc path and proceed with heuristic breakdown.

- Test alignment with default runner wiring
  - Observation: With default `RealCurlRunner` now injected under `timings-curl`, tests that don’t call `with_curl_runner(...)` may inadvertently perform real curl requests.
  - Impact: Flaky/slow tests in CI.
  - Fix: Ensure all `curl_timing_tests` inject `FakeCurlRunner`; or gate default runner injection under `#[cfg(not(test))]`.

---

### Low
- Docs/comments slightly stale
  - Observation: Some docstrings still emphasize “when a runner is injected,” while the runner is now auto‑wired under the feature.
  - Impact: Minor confusion.
  - Fix: Clarify docs to “auto‑wired by default under feature; can be overridden via `with_curl_runner`.”

---

### Acceptance criteria check
- Feature‑gated builds produce numeric phase timings when `timings-curl` is enabled: MET (default `RealCurlRunner` present).
- Default builds unchanged: MET (isahc path intact; heuristic breakdown preserved).
- `breakdown` conforms to `DNS|TCP|TLS|TTFB|Total`; metadata `breakdown_source`, `connection_reused`: MET (format correct; reuse set with tolerant threshold).
- No regression in P95/rolling behavior or status classification: MET (TTFB drives `latency_ms`).

---

### Recommended minimal edits
- Add curl→isahc fallback in `execute_http_probe` on curl errors (resiliency).
- Ensure all curl‑timing tests inject `FakeCurlRunner` (or gate default runner in tests).
- Refresh docs to reflect auto‑wired runner under `timings-curl` and TTFB‑based latency semantics.
