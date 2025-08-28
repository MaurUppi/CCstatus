#!/usr/bin/env bash
set -euo pipefail

# CCStatus E2E harness for COLD/GREEN/RED flows
# Assumptions:
# - Run from ~/Documents/DevProjects/CCstatus (repo root)
# - Binary at ./target/release/ccstatus
# - Transcript exists at the given TRANSCRIPT path
# - jq is installed

# Compute binary path robustly from this script's location (repo-root/target/release/ccstatus)
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
BIN="$REPO_ROOT/target/release/ccstatus"
TRANSCRIPT="/Users/ouzy/.claude/projects/-Users-ouzy-Documents-DevProjects-CCstatus/6ed29d2f-35f2-4cab-9aae-d72eb7907a78.jsonl"
STATE_FILE="$HOME/.claude/ccstatus/ccstatus-monitoring.json"
DEBUG_LOG="$HOME/.claude/ccstatus/ccstatus-debug.log"
SESSION_ID="6ed29d2f-35f2-4cab-9aae-d72eb7907a78"

# Windows
D_GREEN=602500     # (total_duration_ms % 300000) < 3000 => hit GREEN
D_RED=100500       # (total_duration_ms % 10000) < 1000 => hit RED
D_BOTH=600500      # hits both GREEN and RED windows
D_MISS_BOTH=605000 # misses GREEN and RED

require_tools() {
  command -v jq >/dev/null 2>&1 || { echo "jq is required" >&2; exit 1; }
}

check_prereqs() {
  [[ -x "$BIN" ]] || { echo "Binary not found/executable: $BIN" >&2; exit 1; }
  [[ -f "$TRANSCRIPT" ]] || { echo "Transcript not found: $TRANSCRIPT" >&2; exit 1; }
}

reset_state() {
  mkdir -p "$(dirname "$STATE_FILE")"
  rm -f "$STATE_FILE" "$DEBUG_LOG"
}

append_jsonl_error_event() {
  # Append a minimal error record to trigger RED gate
  cat >> "$TRANSCRIPT" <<'EOF'
{"type":"assistant","timestamp":"2025-08-26T01:23:45.000Z","message":{"content":[{"type":"text","text":"API Error: 529 {\"type\":\"error\",\"error\":{\"type\":\"overloaded_error\",\"message\":\"Overloaded\"}}"}]},"isApiErrorMessage":true}
EOF
}

make_stdin_json() {
  local duration_ms="$1"
  jq -n \
    --arg session_id "$SESSION_ID" \
    --arg transcript_path "$TRANSCRIPT" \
    --arg cwd "$PWD" \
    --arg current_dir "$PWD" \
    --arg project_dir "$PWD" \
    --arg version "1.0.89" \
    --arg model_id "claude-sonnet-4-20250514" \
    --arg display_name "Sonnet 4" \
    --arg style_name "default" \
    --argjson total_duration_ms "$duration_ms" \
    '{
      session_id: $session_id,
      transcript_path: $transcript_path,
      cwd: $cwd,
      model: { id: $model_id, display_name: $display_name },
      workspace: { current_dir: $current_dir, project_dir: $project_dir },
      version: $version,
      output_style: { name: $style_name },
      cost: {
        total_cost_usd: 0,
        total_duration_ms: $total_duration_ms,
        total_api_duration_ms: 0,
        total_lines_added: 0,
        total_lines_removed: 0
      },
      exceeds_200k_tokens: false
    }'
}

run_once() {
  # Invokes ccstatus with process-scoped env only (no global pollution)
  # Inputs from caller scope (can be local):
  # - USE_DEBUG=1 to enable debug
  # - TIMEOUT_MS=<int> to set ccstatus_TIMEOUT_MS
  # - BASE_URL, AUTH_TOKEN for credentials
  # - CLEAR_CREDS=1 to force unsetting ANTHROPIC_* for this call
  local duration_ms="$1"

  local env_args=()
  # Force-unset credentials if requested
  if [[ "${CLEAR_CREDS:-0}" == "1" ]]; then
    env_args+=( -u ANTHROPIC_BASE_URL -u ANTHROPIC_AUTH_TOKEN )
  fi
  # Optional assignments
  if [[ "${USE_DEBUG:-0}" == "1" ]]; then
    env_args+=( CCSTATUS_DEBUG=true )
  fi
  if [[ "${CCSTATUS_NO_CREDENTIALS:-0}" == "1" ]]; then
    env_args+=( CCSTATUS_NO_CREDENTIALS=1 )
  fi
  if [[ -n "${TIMEOUT_MS:-}" ]]; then
    env_args+=( ccstatus_TIMEOUT_MS="$TIMEOUT_MS" )
  fi
  if [[ -n "${BASE_URL:-}" ]]; then
    env_args+=( ANTHROPIC_BASE_URL="$BASE_URL" )
  fi
  if [[ -n "${AUTH_TOKEN:-}" ]]; then
    env_args+=( ANTHROPIC_AUTH_TOKEN="$AUTH_TOKEN" )
  fi

  make_stdin_json "$duration_ms" | env "${env_args[@]}" "$BIN"
}

show_state() {
  if [[ -f "$STATE_FILE" ]]; then
    echo "\n--- state ($STATE_FILE) ---"
    jq '.' "$STATE_FILE" || true
  else
    echo "No state file at $STATE_FILE"
  fi
}

assert_jq() {
  local filter="$1"; shift
  jq -e "$filter" "$STATE_FILE" >/dev/null || {
    echo "Assertion failed: jq '$filter'" >&2
    jq '.' "$STATE_FILE" || true
    exit 1
  }
}

get_rolling_len() {
  if [[ -f "$STATE_FILE" ]]; then
    jq -r '(.network.rolling_totals // []) | length' "$STATE_FILE"
  else
    echo 0
  fi
}

get_last_http_status() {
  if [[ -f "$STATE_FILE" ]]; then
    jq -r '(.network.last_http_status // -1)' "$STATE_FILE"
  else
    echo -1
  fi
}

get_state_timestamp() {
  if [[ -f "$STATE_FILE" ]]; then
    jq -r '(.timestamp // "")' "$STATE_FILE"
  else
    echo ""
  fi
}

wait_for_debug_log() {
  # small wait to allow async sidecar to flush
  for _ in {1..10}; do
    [[ -f "$DEBUG_LOG" ]] && return 0
    sleep 0.1
  done
  return 1
}

assert_debug_contains() {
  local needle="$1"
  wait_for_debug_log || { echo "Debug log not found at $DEBUG_LOG" >&2; return 1; }
  grep -q "$needle" "$DEBUG_LOG" || {
    echo "Expected debug log to contain: $needle" >&2
    tail -n +200 "$DEBUG_LOG" || true
    exit 1
  }
}

# Wait for state file to appear (in case writer is slightly delayed)
wait_for_state_file() {
  local tries="${1:-20}"
  local sleep_ms="${2:-100}"
  for ((i=0;i<tries;i++)); do
    [[ -f "$STATE_FILE" ]] && return 0
    # macOS sleep supports fractional seconds; compute ms -> seconds
    sleep "$(awk -v ms=$sleep_ms 'BEGIN{printf "%.3f", ms/1000.0}')"
  done
  return 1
}

scenario_unknown() {
  echo "[Scenario] Unknown credentials"
  reset_state
  CLEAR_CREDS=1 CCSTATUS_NO_CREDENTIALS=1 USE_DEBUG=1 run_once "$D_GREEN"
  # The implementation SHOULD write unknown state. If not present, error with guidance.
  if ! wait_for_state_file 30 100; then
    echo "Error: state file was not created by ccstatus for unknown-credentials path: $STATE_FILE" >&2
    echo "Hint: According to spec, NetworkSegment should call HttpMonitor::write_unknown when creds are absent." >&2
    exit 1
  fi
  show_state
  assert_jq '.status == "Unknown" and .monitoring_enabled == false'
  echo "OK: status unknown; monitoring disabled"
}

scenario_cold() {
  echo "[Scenario] COLD (first run with creds)"
  reset_state
  USE_DEBUG=1 BASE_URL="${ANTHROPIC_BASE_URL:-https://api.anthropic.com}" AUTH_TOKEN="${ANTHROPIC_AUTH_TOKEN:-invalid-token-for-e2e}" \
    run_once "$D_MISS_BOTH"   # any value; COLD ignores windows
  wait_for_state_file 30 100 || { echo "Error: state file not created on COLD path" >&2; exit 1; }
  show_state
  assert_jq ".monitoring_state.last_cold_session_id == \"$SESSION_ID\""
  assert_jq '.monitoring_state.last_cold_probe_at | type == "string" and length > 0'
  # Local time format with offset (e.g., +08:00)
  assert_jq '.monitoring_state.last_cold_probe_at | test("^[0-9]{4}-[0-9]{2}-[0-9]{2}T.*[+-][0-9]{2}:[0-9]{2}$")'
  assert_jq '.timestamp | test("^[0-9]{4}-[0-9]{2}-[0-9]{2}T.*[+-][0-9]{2}:[0-9]{2}$")'
  echo "OK: COLD markers present"
}

scenario_green() {
  echo "[Scenario] GREEN window"
  local before_len; before_len=$(get_rolling_len)
  USE_DEBUG=1 BASE_URL="${ANTHROPIC_BASE_URL:-https://api.anthropic.com}" AUTH_TOKEN="${ANTHROPIC_AUTH_TOKEN:-invalid-token-for-e2e}" \
    run_once "$D_GREEN"
  wait_for_state_file 30 100 || { echo "Error: state file not created on GREEN path" >&2; exit 1; }
  show_state
  local after_len; after_len=$(get_rolling_len)
  local status_code; status_code=$(get_last_http_status)

  if [[ "$status_code" -eq 200 ]]; then
    [[ "$after_len" -eq $((before_len + 1)) ]] || { echo "Expected rolling_totals to grow on 200" >&2; exit 1; }
    echo "OK: GREEN success appended rolling_totals (len $before_len -> $after_len)"
  else
    [[ "$after_len" -eq "$before_len" ]] || { echo "Expected rolling_totals unchanged on non-200" >&2; exit 1; }
    echo "OK: GREEN non-200 did not change rolling_totals (len $before_len)"
  fi
}

scenario_green_skip() {
  echo "[Scenario] GREEN window skip (no write)"
  local ts_before; ts_before=$(get_state_timestamp || true)
  USE_DEBUG=1 BASE_URL="${ANTHROPIC_BASE_URL:-https://api.anthropic.com}" AUTH_TOKEN="${ANTHROPIC_AUTH_TOKEN:-invalid-token-for-e2e}" \
    run_once "$D_MISS_BOTH" # miss both windows
  # No write expected; do not wait
  local ts_after; ts_after=$(get_state_timestamp || true)
  # If no prior state, skip assert
  if [[ -n "${ts_before:-}" ]]; then
    [[ "$ts_after" == "$ts_before" ]] || { echo "Expected state timestamp unchanged when skipping windows" >&2; exit 1; }
    echo "OK: no write when window missed"
  else
    echo "Note: no prior state to compare; skip check"
  fi
}

scenario_red_hit() {
  echo "[Scenario] RED window (error-driven)"
  append_jsonl_error_event
  USE_DEBUG=1 BASE_URL="${ANTHROPIC_BASE_URL:-https://api.anthropic.com}" AUTH_TOKEN="${ANTHROPIC_AUTH_TOKEN:-invalid-token-for-e2e}" \
    run_once "$D_RED"
  wait_for_state_file 30 100 || { echo "Error: state file not created on RED path" >&2; exit 1; }
  show_state
  assert_jq '.last_jsonl_error_event != null'
  assert_jq '.status == "Error"'
  echo "OK: RED wrote last_jsonl_error_event and status=error"
}

scenario_red_skip() {
  echo "[Scenario] RED gate skipped when window misses"
  local ts_before; ts_before=$(get_state_timestamp || true)
  append_jsonl_error_event
  USE_DEBUG=1 BASE_URL="${ANTHROPIC_BASE_URL:-https://api.anthropic.com}" AUTH_TOKEN="${ANTHROPIC_AUTH_TOKEN:-invalid-token-for-e2e}" \
    run_once "$D_MISS_BOTH" # miss red and green
  local ts_after; ts_after=$(get_state_timestamp || true)
  if [[ -n "${ts_before:-}" ]]; then
    [[ "$ts_after" == "$ts_before" ]] || { echo "Expected no write when RED missed and GREEN missed" >&2; exit 1; }
    echo "OK: no write on RED miss"
  else
    echo "Note: no prior state to compare; skip check"
  fi
}

scenario_priority() {
  echo "[Scenario] Priority COLD > RED > GREEN"
  # First ensure state exists (COLD once)
  reset_state
  USE_DEBUG=1 BASE_URL="${ANTHROPIC_BASE_URL:-https://api.anthropic.com}" AUTH_TOKEN="${ANTHROPIC_AUTH_TOKEN:-invalid-token-for-e2e}" \
    run_once "$D_MISS_BOTH"
  wait_for_state_file 30 100 || { echo "Error: state file not created on COLD init path" >&2; exit 1; }
  assert_jq '.monitoring_state.last_cold_session_id != null'

  # Now append error so RED gate can be considered, and choose a duration that hits both windows
  append_jsonl_error_event
  local before_len; before_len=$(get_rolling_len)
  USE_DEBUG=1 BASE_URL="${ANTHROPIC_BASE_URL:-https://api.anthropic.com}" AUTH_TOKEN="${ANTHROPIC_AUTH_TOKEN:-invalid-token-for-e2e}" \
    run_once "$D_BOTH"
  wait_for_state_file 30 100 || { echo "Error: state file not created when both windows hit" >&2; exit 1; }
  show_state

  # Expect RED took priority over GREEN (no rolling_totals change, and error event present)
  assert_jq '.last_jsonl_error_event != null'
  local after_len; after_len=$(get_rolling_len)
  [[ "$after_len" -eq "$before_len" ]] || { echo "Expected no rolling_totals update when RED executed (priority over GREEN)" >&2; exit 1; }
  echo "OK: RED prioritized over GREEN"
}

scenario_timeout_override() {
  echo "[Scenario] Timeout override (ccstatus_TIMEOUT_MS)"
  USE_DEBUG=1 TIMEOUT_MS=1800 BASE_URL="${ANTHROPIC_BASE_URL:-https://api.anthropic.com}" AUTH_TOKEN="${ANTHROPIC_AUTH_TOKEN:-invalid-token-for-e2e}" \
    run_once "$D_GREEN"
  wait_for_state_file 30 100 || { echo "Error: state file not created with timeout override" >&2; exit 1; }
  show_state
  # Best-effort: check debug mentions 1800
  if [[ -f "$DEBUG_LOG" ]] && grep -q "1800" "$DEBUG_LOG"; then
    echo "OK: debug log shows timeout override"
  else
    echo "Note: could not confirm timeout in debug log (non-fatal)"
  fi
}

# Extra scenarios (best-effort)

scenario_env_source() {
  echo "[Scenario] Env credentials source recorded"
  reset_state
  USE_DEBUG=1 BASE_URL="${ANTHROPIC_BASE_URL:-https://api.anthropic.com}" AUTH_TOKEN="${ANTHROPIC_AUTH_TOKEN:-invalid-token-for-e2e}" \
    run_once "$D_GREEN"
  wait_for_state_file 30 100 || { echo "Error: state file not created for env-source scenario" >&2; exit 1; }
  show_state
  assert_jq '.api_config.source == "environment"'
  echo "OK: api_config.source=environment"
}

scenario_rolling_cap() {
  echo "[Scenario] Rolling totals cap (<=12)"
  # Try many GREEN runs; only 200s will append
  for i in {1..15}; do
    USE_DEBUG=1 BASE_URL="${ANTHROPIC_BASE_URL:-https://api.anthropic.com}" AUTH_TOKEN="${ANTHROPIC_AUTH_TOKEN:-invalid-token-for-e2e}" \
      run_once "$D_GREEN"
    wait_for_state_file 30 100 || { echo "Error: state file not created on GREEN loop" >&2; exit 1; }
  done
  show_state
  # Cap should never exceed 12
  assert_jq '((.network.rolling_totals // []) | length) <= 12'
  echo "OK: rolling_totals length <= 12"
}

scenario_renderer_smoke() {
  echo "[Scenario] Renderer smoke (emoji present on stdout)"
  local out
  out=$(USE_DEBUG=1 BASE_URL="${ANTHROPIC_BASE_URL:-https://api.anthropic.com}" AUTH_TOKEN="${ANTHROPIC_AUTH_TOKEN:-invalid-token-for-e2e}" make_stdin_json "$D_GREEN" | "$BIN")
  if echo "$out" | grep -q -E "[🟢🟡🔴⚪]"; then
    echo "OK: statusline emitted emoji"
  else
    echo "Note: could not detect emoji in output (non-fatal)"
  fi
}

usage() {
  cat <<USAGE
Usage: $0 [all|unknown|cold|green|green-skip|red-hit|red-skip|priority|timeout|env-source|rolling-cap|renderer]
Default: all
USAGE
}

main() {
  require_tools
  check_prereqs

  local target="${1:-all}"
  case "$target" in
    all)
      scenario_unknown
      scenario_cold
      scenario_green
      scenario_green_skip
      scenario_red_hit
      scenario_red_skip
      scenario_priority
      scenario_timeout_override
      scenario_env_source
      scenario_rolling_cap
      scenario_renderer_smoke
      ;;
    unknown) scenario_unknown ;;
    cold) scenario_cold ;;
    green) scenario_green ;;
    green-skip) scenario_green_skip ;;
    red-hit) scenario_red_hit ;;
    red-skip) scenario_red_skip ;;
    priority) scenario_priority ;;
    timeout) scenario_timeout_override ;;
    env-source) scenario_env_source ;;
    rolling-cap) scenario_rolling_cap ;;
    renderer) scenario_renderer_smoke ;;
    *) usage; exit 1 ;;
  esac

  echo "\nAll done."
}

main "$@"
