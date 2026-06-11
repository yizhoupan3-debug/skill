#!/usr/bin/env bash
# Subagent health monitor — tracks spawn/stop, detects stuck agents, auto-terminates.
# Called from SubagentStart/SubagentStop hooks and on-demand health checks.
#
# Usage:
#   agent-health-monitor.sh start <agent_name> <agent_id>   # record spawn
#   agent-health-monitor.sh stop  <agent_id>                 # record stop
#   agent-health-monitor.sh check                            # health report (JSON)
#   agent-health-monitor.sh kill-stuck [timeout_secs]        # auto-terminate stuck agents
set -euo pipefail

STATE_DIR="${CLAUDE_PROJECT_ROOT:-$(git rev-parse --show-toplevel 2>/dev/null || pwd)}/.claude"
STATE_FILE="$STATE_DIR/agent_health_state.json"
DEFAULT_TIMEOUT_SECS=600  # 10 minutes

mkdir -p "$STATE_DIR"

# Initialize state file if missing
init_state() {
  if [ ! -f "$STATE_FILE" ]; then
    echo '{"agents":{},"last_check":""}' > "$STATE_FILE"
  fi
}

# Record agent start
do_start() {
  local name="${1:?agent name required}"
  local agent_id="${2:?agent id required}"
  init_state
  local now
  now=$(date -u +"%Y-%m-%dT%H:%M:%SZ")

  python3 -c "
import json, sys
with open('$STATE_FILE', 'r') as f:
    state = json.load(f)
state['agents']['$agent_id'] = {
    'name': '$name',
    'started_at': '$now',
    'status': 'running',
    'last_heartbeat': '$now'
}
with open('$STATE_FILE', 'w') as f:
    json.dump(state, f, indent=2)
print(json.dumps({'event': 'agent_start', 'agent_id': '$agent_id', 'name': '$name'}))
"
}

# Record agent stop
do_stop() {
  local agent_id="${1:?agent id required}"
  init_state
  local now
  now=$(date -u +"%Y-%m-%dT%H:%M:%SZ")

  python3 -c "
import json
with open('$STATE_FILE', 'r') as f:
    state = json.load(f)
if '$agent_id' in state['agents']:
    agent = state['agents']['$agent_id']
    agent['status'] = 'stopped'
    agent['stopped_at'] = '$now'
with open('$STATE_FILE', 'w') as f:
    json.dump(state, f, indent=2)
print(json.dumps({'event': 'agent_stop', 'agent_id': '$agent_id'}))
"
}

# Health check — returns JSON with agent statuses and warnings
do_check() {
  init_state
  local timeout_secs="${1:-$DEFAULT_TIMEOUT_SECS}"

  python3 -c "
import json, sys
from datetime import datetime, timezone

timeout = int('$timeout_secs')
with open('$STATE_FILE', 'r') as f:
    state = json.load(f)

now = datetime.now(timezone.utc)
report = {'healthy': True, 'agents': [], 'stuck': [], 'timeout_secs': timeout}

for aid, agent in state.get('agents', {}).items():
    if agent.get('status') != 'running':
        continue
    started = datetime.fromisoformat(agent['started_at'].replace('Z', '+00:00'))
    age_secs = int((now - started).total_seconds())
    entry = {
        'agent_id': aid,
        'name': agent.get('name', 'unknown'),
        'age_secs': age_secs,
        'age_human': f'{age_secs // 60}m{age_secs % 60}s',
    }
    report['agents'].append(entry)
    if age_secs > timeout:
        entry['status'] = 'STUCK'
        report['stuck'].append(aid)
        report['healthy'] = False

report['active_count'] = len(report['agents'])
report['stuck_count'] = len(report['stuck'])
state['last_check'] = now.isoformat().replace('+00:00', 'Z')
with open('$STATE_FILE', 'w') as f:
    json.dump(state, f, indent=2)
print(json.dumps(report, indent=2))
"
}

# Kill stuck agents by sending MCP terminate
do_kill_stuck() {
  local timeout_secs="${1:-$DEFAULT_TIMEOUT_SECS}"
  local report
  report=$(do_check "$timeout_secs")
  local stuck_count
  stuck_count=$(echo "$report" | python3 -c "import json,sys; print(json.load(sys.stdin).get('stuck_count',0))")

  if [ "$stuck_count" -eq 0 ]; then
    echo '{"action":"none","reason":"no stuck agents"}'
    return 0
  fi

  echo "$report" | python3 -c "
import json, sys
report = json.load(sys.stdin)
actions = []
for aid in report.get('stuck', []):
    agent = next((a for a in report['agents'] if a['agent_id'] == aid), {})
    actions.append({
        'action': 'terminate',
        'agent_id': aid,
        'name': agent.get('name', 'unknown'),
        'age_secs': agent.get('age_secs', 0),
    })
print(json.dumps({'action': 'terminate_stuck', 'count': len(actions), 'agents': actions}, indent=2))
"
}

# Clean up old entries (> 1 hour)
do_cleanup() {
  init_state
  python3 -c "
import json
from datetime import datetime, timezone, timedelta

with open('$STATE_FILE', 'r') as f:
    state = json.load(f)
now = datetime.now(timezone.utc)
cutoff = now - timedelta(hours=1)
before = len(state.get('agents', {}))
state['agents'] = {
    k: v for k, v in state.get('agents', {}).items()
    if v.get('status') == 'running' or
       datetime.fromisoformat(v.get('stopped_at', '2099-01-01T00:00:00Z').replace('Z', '+00:00')) > cutoff
}
after = len(state['agents'])
with open('$STATE_FILE', 'w') as f:
    json.dump(state, f, indent=2)
print(json.dumps({'cleaned': before - after, 'remaining': after}))
"
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
