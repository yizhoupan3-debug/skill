#!/usr/bin/env bash
# Workspace bootstrap template must match repo .cursor/hooks.json (7-event subtraction set).
# Event lists are loaded from `router-rs schema-drift contract` (single source with subtraction.rs).
# Dependencies: jq (no python).
set -euo pipefail
root="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$root"
hooks="$root/.cursor/hooks.json"
template="$root/configs/framework/cursor-hooks.workspace-template.json"
for f in "$hooks" "$template"; do
  [[ -f "$f" ]] || { echo "FAIL: missing $f"; exit 1; }
done

# Load contract from router-rs
contract=$(cargo run --quiet --manifest-path core/router-rs/Cargo.toml -- schema-drift contract 2>/dev/null) || {
  echo "FAIL: cargo schema-drift contract failed" >&2
  exit 1
}
required=$(echo "$contract" | jq -r '.cursor_hooks_required[]')
forbidden=$(echo "$contract" | jq -r '.cursor_hooks_forbidden[]')

# Build timeout expectations as JSON
gate_timeouts='{
  "beforeSubmitPrompt": 20, "stop": 20, "postToolUse": 20,
  "subagentStart": 20, "subagentStop": 20, "sessionStart": 5, "sessionEnd": 15
}'

# Validate a hooks JSON file against contract
validate_hooks() {
  local label="$1" file="$2"
  local h
  h=$(jq '.hooks // {}' "$file")
  local keys
  keys=$(echo "$h" | jq -r 'keys[]')
  local errs=""

  # Check required events
  for ev in $required; do
    if ! echo "$keys" | grep -qx "$ev"; then
      errs="${errs}${label}: missing required event ${ev}\n"
    fi
  done

  # Check forbidden events
  for ev in $forbidden; do
    if echo "$keys" | grep -qx "$ev"; then
      errs="${errs}${label}: forbidden removed event ${ev} still registered\n"
    fi
  done

  # Check timeouts and commands
  for ev in beforeSubmitPrompt stop postToolUse subagentStart subagentStop sessionStart sessionEnd; do
    local want
    want=$(echo "$gate_timeouts" | jq -r --arg ev "$ev" '.[$ev] // empty')
    [[ -z "$want" ]] && continue
    local actual_timeout
    actual_timeout=$(echo "$h" | jq -r --arg ev "$ev" '.[$ev][0].timeout // empty')
    if [[ -n "$actual_timeout" && "$actual_timeout" != "$want" ]]; then
      errs="${errs}${label}: ${ev} timeout must be ${want}s (got ${actual_timeout})\n"
    fi
    local cmd
    cmd=$(echo "$h" | jq -r --arg ev "$ev" '.[$ev][0].command // empty')
    if [[ -n "$cmd" && "$cmd" != *"cursor-router-rs-hook.sh"* ]]; then
      errs="${errs}${label}: ${ev} must invoke cursor-router-rs-hook.sh\n"
    fi
  done

  printf '%s' "$errs"
}

errs=""
errs+=$(validate_hooks ".cursor/hooks.json" "$hooks")
errs+=$(validate_hooks "workspace-template" "$template")

# Check key mismatch
h_keys=$(jq -r '.hooks // {} | keys | sort | join(",")' "$hooks")
t_keys=$(jq -r '.hooks // {} | keys | sort | join(",")' "$template")
if [[ "$h_keys" != "$t_keys" ]]; then
  errs="${errs}event key mismatch: hooks=[${h_keys}] template=[${t_keys}]\n"
else
  # Check timeout and command parity between hooks and template
  for ev in $(echo "$h_keys" | tr ',' '\n'); do
    h_to=$(jq -r --arg ev "$ev" '.hooks[$ev][0].timeout // empty' "$hooks")
    t_to=$(jq -r --arg ev "$ev" '.hooks[$ev][0].timeout // empty' "$template")
    if [[ "$h_to" != "$t_to" ]]; then
      errs="${errs}timeout mismatch on ${ev}: hooks=${h_to} template=${t_to}\n"
    fi
    h_cmd=$(jq -r --arg ev "$ev" '.hooks[$ev][0].command // empty' "$hooks")
    t_cmd=$(jq -r --arg ev "$ev" '.hooks[$ev][0].command // empty' "$template")
    if [[ "$h_cmd" != "$t_cmd" ]]; then
      errs="${errs}command mismatch on ${ev}\n"
    fi
  done
fi

if [[ -n "$errs" ]]; then
  printf '%b' "$errs" >&2
  exit 1
fi
event_count=$(echo "$required" | wc -l | tr -d ' ')
echo "OK: .cursor/hooks.json matches cursor-hooks.workspace-template.json (${event_count} events, contract-driven lists)"
