#!/usr/bin/env bash
# Subagent health monitor — tracks spawn/stop, detects stuck agents, auto-terminates.
# Called from SubagentStart/SubagentStop hooks and on-demand health checks.
# Dependencies: jq (replaces prior python3 usage).
#
# Usage:
#   agent-health-monitor.sh start <agent_name> <agent_id>   # record spawn
#   agent-health-monitor.sh stop  <agent_id>                 # record stop
#   agent-health-monitor.sh check                            # health report (JSON)
#   agent-health-monitor.sh kill-stuck [timeout_secs]        # auto-terminate stuck agents
set -euo pipefail

# Detect repo root from any host's environment, then fall back to git.
_REPO_ROOT="${CLAUDE_PROJECT_ROOT:-${CODEX_PROJECT_ROOT:-${CURSOR_WORKSPACE_ROOT:-${OPENCODE_PROJECT_ROOT:-$(git rev-parse --show-toplevel 2>/dev/null || pwd)}}}}"
# Use .framework/agent-health/ for host-agnostic state (previously .claude/).
STATE_DIR="${_REPO_ROOT}/.framework/agent-health"
STATE_FILE="$STATE_DIR/agent_health_state.json"
LOCK_FILE="$STATE_DIR/agent_health_state.lock"
DEFAULT_TIMEOUT_SECS=600  # 10 minutes

# One-time migration: move old .claude/ state to .framework/agent-health/
_OLD_STATE_DIR="${_REPO_ROOT}/.claude"
if [ -f "$_OLD_STATE_DIR/agent_health_state.json" ] && [ ! -f "$STATE_FILE" ]; then
  mkdir -p "$STATE_DIR"
  mv "$_OLD_STATE_DIR/agent_health_state.json" "$STATE_FILE" 2>/dev/null || true
  rm -f "$_OLD_STATE_DIR/agent_health_state.lock" 2>/dev/null || true
fi

mkdir -p "$STATE_DIR"

# Acquire exclusive flock with 10s timeout.
# Opened on fd 9; released on script exit.
acquire_lock() {
  exec 9>"$LOCK_FILE"
  flock --exclusive --timeout 10 9 || {
    echo '{"error":"failed to acquire agent health lock within 10s"}' >&2
    exit 1
  }
}

# Initialize state file if missing
init_state() {
  if [ ! -f "$STATE_FILE" ]; then
    echo '{"agents":{},"last_check":""}' > "$STATE_FILE"
  fi
}

# Record agent start
do_start() {
  acquire_lock
  local name="${1:?agent name required}"
  local agent_id="${2:?agent id required}"
  init_state
  local now
  now=$(date -u +"%Y-%m-%dT%H:%M:%SZ")

  local tmp="${STATE_FILE}.tmp.$$"
  jq --arg name "$name" --arg aid "$agent_id" --arg now "$now" \
    '.agents[$aid] = {"name": $name, "started_at": $now, "status": "running", "last_heartbeat": $now}' \
    "$STATE_FILE" > "$tmp" && mv "$tmp" "$STATE_FILE"

  jq -n --arg name "$name" --arg aid "$agent_id" '{event:"agent_start",agent_id:$aid,name:$name}'
}

# Record agent stop
do_stop() {
  acquire_lock
  local agent_id="${1:?agent id required}"
  init_state
  local now
  now=$(date -u +"%Y-%m-%dT%H:%M:%SZ")

  local tmp="${STATE_FILE}.tmp.$$"
  jq --arg aid "$agent_id" --arg now "$now" \
    'if .agents[$aid] then .agents[$aid].status = "stopped" | .agents[$aid].stopped_at = $now else . end' \
    "$STATE_FILE" > "$tmp" && mv "$tmp" "$STATE_FILE"

  jq -n --arg aid "$agent_id" '{event:"agent_stop",agent_id:$aid}'
}

# Health check — returns JSON with agent statuses and warnings (single file read for atomicity)
do_check() {
  acquire_lock
  init_state
  local timeout_secs="${1:-$DEFAULT_TIMEOUT_SECS}"
  local now
  now=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
  local tmp="${STATE_FILE}.tmp.$$"

  local combined
  combined=$(jq --argjson timeout "$timeout_secs" --arg now "$now" '
    (.agents // {}) as $agents |
    ([$now | strptime("%Y-%m-%dT%H:%M:%SZ") | mktime] | .[0]) as $now_epoch |
    [
      $agents | to_entries[] |
      select(.value.status == "running") |
      (.value.started_at | strptime("%Y-%m-%dT%H:%M:%SZ") | mktime) as $started |
      {
        agent_id: .key,
        name: (.value.name // "unknown"),
        age_secs: (($now_epoch - $started) | floor),
        age_human: (((($now_epoch - $started) | floor) / 60 | floor | tostring) + "m" + ((($now_epoch - $started) | floor) % 60 | tostring) + "s")
      }
    ] as $active |
    [$active[] | select(.age_secs > $timeout)] as $stuck |
    {
      _updated_state: (.last_check = $now),
      report: {
        healthy: ($stuck | length == 0),
        agents: $active,
        stuck: [$stuck[].agent_id],
        timeout_secs: $timeout,
        active_count: ($active | length),
        stuck_count: ($stuck | length)
      }
    }
  ' "$STATE_FILE")

  printf '%s' "$combined" | jq '._updated_state' > "$tmp" && mv "$tmp" "$STATE_FILE"
  printf '%s' "$combined" | jq '.report'
}

# Kill stuck agents by sending MCP terminate
do_kill_stuck() {
  local timeout_secs="${1:-$DEFAULT_TIMEOUT_SECS}"
  local report
  report=$(do_check "$timeout_secs")
  local stuck_count
  stuck_count=$(printf '%s' "$report" | jq -r '.stuck_count')

  if [ "$stuck_count" -eq 0 ]; then
    echo '{"action":"none","reason":"no stuck agents"}'
    return 0
  fi

  printf '%s' "$report" | jq '{
    action: "terminate_stuck",
    count: (.stuck | length),
    agents: [
      .stuck[] as $aid |
      (.agents[] | select(.agent_id == $aid)) // {} |
      {
        action: "terminate",
        agent_id: $aid,
        name: (.name // "unknown"),
        age_secs: (.age_secs // 0)
      }
    ]
  }'
}

# Clean up old entries (> 1 hour)
do_cleanup() {
  acquire_lock
  init_state
  local now_epoch
  now_epoch=$(date -u +%s)
  local cutoff=$((now_epoch - 3600))

  local before after tmp="${STATE_FILE}.tmp.$$"
  before=$(jq '.agents | length' "$STATE_FILE")

  jq --argjson cutoff "$cutoff" '
    .agents |= with_entries(
      select(
        .value.status == "running" or
        (.value.stopped_at // "2099-01-01T00:00:00Z" | strptime("%Y-%m-%dT%H:%M:%SZ") | mktime) > $cutoff
      )
    )
  ' "$STATE_FILE" > "$tmp" && mv "$tmp" "$STATE_FILE"

  after=$(jq '.agents | length' "$STATE_FILE")
  printf '%s\n' "{\"cleaned\":$((before - after)),\"remaining\":$after}"
}

# Main dispatch
case "${1:-help}" in
  start)    do_start "${2:?}" "${3:?}" ;;
  stop)     do_stop "${2:?}" ;;
  check)    do_check "${2:-$DEFAULT_TIMEOUT_SECS}" ;;
  kill-stuck) do_kill_stuck "${2:-$DEFAULT_TIMEOUT_SECS}" ;;
  cleanup)  do_cleanup ;;
  *)
    echo "Usage: $0 {start|stop|check|kill-stuck|cleanup} [args...]"
    exit 1
    ;;
esac
