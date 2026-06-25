#!/usr/bin/env bash
# Subagent health monitor — tracks spawn/stop, records agent lifecycle events.
# Called from configs/framework/hook.sh on SubagentStart/SubagentStop.
# Dependencies: jq (replaces prior python3 usage).
#
# Usage:
#   agent-health-monitor.sh start <agent_name> <agent_id>   # record spawn
#   agent-health-monitor.sh stop  <agent_id>                 # record stop
set -euo pipefail

# Detect repo root from any host's environment, then fall back to git.
_REPO_ROOT="${CLAUDE_PROJECT_ROOT:-${CODEX_PROJECT_ROOT:-${CURSOR_WORKSPACE_ROOT:-${OPENCODE_PROJECT_ROOT:-$(git rev-parse --show-toplevel 2>/dev/null || pwd)}}}}"
# Use .framework/agent-health/ for host-agnostic state (previously .claude/).
STATE_DIR="${_REPO_ROOT}/.framework/agent-health"
STATE_FILE="$STATE_DIR/agent_health_state.json"
LOCK_FILE="$STATE_DIR/agent_health_state.lock"

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

# Main dispatch
case "${1:-help}" in
  start)    do_start "${2:?}" "${3:?}" ;;
  stop)     do_stop "${2:?}" ;;
  *)
    echo "Usage: $0 {start|stop} <args...>"
    exit 1
    ;;
esac
