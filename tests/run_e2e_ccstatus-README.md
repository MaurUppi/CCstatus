### CCStatus E2E Harness for Network Monitoring (COLD/GREEN/RED)

This README documents how to run the end-to-end test harness that exercises stdin → orchestration (COLD/RED/GREEN) → HttpMonitor probe → single-writer state persistence → StatusRenderer output, aligned with `project/network/1-network_monitoring-Requirement-Final.md`.

Files:
- `tests/run_e2e_ccstatus.sh`: executable test script
- `tests/3-network_monitoring-e2e-test-proposal.md`: full test plan and acceptance criteria

Prerequisites:
- macOS with Bash
- `jq` installed (brew install jq)
- Built binary at `./target/release/ccstatus`
- Real transcript JSONL at:
  `/Users/ouzy/.claude/projects/-Users-ouzy-Documents-DevProjects-CCstatus/6ed29d2f-35f2-4cab-9aae-d72eb7907a78.jsonl`

Run all scenarios:
```bash
cd ~/Documents/DevProjects/CCstatus
chmod +x tests/run_e2e_ccstatus.sh
tests/run_e2e_ccstatus.sh
```

Run one scenario:
```bash
tests/run_e2e_ccstatus.sh unknown
tests/run_e2e_ccstatus.sh cold
tests/run_e2e_ccstatus.sh green
tests/run_e2e_ccstatus.sh green-skip
tests/run_e2e_ccstatus.sh red-hit
tests/run_e2e_ccstatus.sh red-skip
tests/run_e2e_ccstatus.sh priority
tests/run_e2e_ccstatus.sh timeout
tests/run_e2e_ccstatus.sh env-source
tests/run_e2e_ccstatus.sh rolling-cap
tests/run_e2e_ccstatus.sh renderer
```

Ephemeral credentials (no global env pollution):
```bash
# Provide valid creds only for this script invocation (and its children)
ANTHROPIC_BASE_URL="https://api.anthropic.com" \
ANTHROPIC_AUTH_TOKEN="<your_token>" \
tests/run_e2e_ccstatus.sh green

# Or for all scenarios in a single run
ANTHROPIC_BASE_URL="https://api.anthropic.com" \
ANTHROPIC_AUTH_TOKEN="<your_token>" \
tests/run_e2e_ccstatus.sh
```

What each scenario covers (mapping to the proposal):
- unknown: Proposal 3.1 (no credentials)
- cold: Proposal 3.3–3.5 (COLD path, de-dupe markers, local time format)
- green: Proposal 3.6–3.8 (GREEN window, rolling_totals append on 200)
- green-skip: Proposal 3.9 (missed GREEN results in no write)
- red-hit: Proposal 3.10 (error in transcript + RED window → error state, last_jsonl_error_event)
- red-skip: Proposal 3.11 (error present but window miss → no write)
- priority: Proposal 3.12 (COLD > RED > GREEN)
- timeout: Proposal 3.14 (`ccstatus_TIMEOUT_MS` override, best-effort debug check)
- env-source: Proposal 3.2 (api_config.source = environment)
- rolling-cap: Proposal 3.13 (cap rolling_totals to 12)
- renderer: Proposal 3.17 (StatusRenderer emoji smoke check)

Notes:
- The script uses the real transcript file and appends a synthetic error line for RED tests. If undesirable, duplicate the JSONL elsewhere and update the `TRANSCRIPT` variable in the script.
- Default token is `invalid-token-for-e2e`. To test 200-success attaches to rolling_totals, pass valid creds inline to the command (examples above). The harness injects them process‑locally and does not export globally.
- State is written to `~/.claude/ccstatus/ccstatus-monitoring.json`. The script resets it between scenarios.
- Debug is enabled per child process; no need to export globally.

Environment isolation:
- The harness never exports `ANTHROPIC_BASE_URL` or `ANTHROPIC_AUTH_TOKEN`.
- Each ccstatus invocation receives credentials and flags via process‑scoped env only.
- The "unknown" scenario force‑unsets credentials for its call without changing your shell.
