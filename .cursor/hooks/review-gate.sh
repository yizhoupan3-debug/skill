#!/usr/bin/env bash
# Cursor review / autopilot gate — forwards stdin JSON to router-rs `cursor hook`.
#
# Usage: review-gate.sh <EventName>
# Events: BeforeSubmitPrompt, Stop, PostToolUse, SubagentStart, SubagentStop,
#         AfterAgentResponse, PreCompact, SessionEnd
#
# When router-rs is missing or not executable: 默认 fail-open（stdout "{}"）；设置
# ROUTER_RS_CURSOR_HOOK_STRICT=1 时 fail-closed（stderr 说明 + stdout JSON 提示 + 非零退出）。

set -euo pipefail

EVENT="${1:?review-gate.sh: event name required (e.g. BeforeSubmitPrompt)}"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
INPUT="$(cat)"
repo_root="$("${SCRIPT_DIR}/resolve-repo-root.sh")"

resolve_router_rs() {
  local root="$1"
  local c
  for c in \
    "$root/scripts/router-rs/target/release/router-rs" \
    "$root/scripts/router-rs/target/debug/router-rs" \
    "$root/target/release/router-rs" \
    "$root/target/debug/router-rs"; do
    if [[ -x "$c" ]]; then
      printf '%s' "$c"
      return 0
    fi
  done
  command -v router-rs 2>/dev/null || true
}

ROUTER_BIN="$(resolve_router_rs "$repo_root")"

if [[ -z "${ROUTER_BIN}" || ! -x "${ROUTER_BIN}" ]]; then
  msg="review-gate: router-rs binary not found under repo or PATH; review/autopilot gate disabled. Run: (cd scripts/router-rs && cargo build --release)"
  echo "$msg" >&2
  if [[ "${ROUTER_RS_CURSOR_HOOK_STRICT:-}" == "1" ]]; then
    ctx="${msg} (fail-closed: ROUTER_RS_CURSOR_HOOK_STRICT=1)"
    if command -v jq &>/dev/null; then
      jq -n --arg c "$ctx" '{additional_context: $c}'
    else
      python3 -c 'import json,sys; print(json.dumps({"additional_context": sys.argv[1]}))' "$ctx"
    fi
    exit 1
  fi
  echo '{}'
  exit 0
fi

printf '%s' "$INPUT" | "${ROUTER_BIN}" cursor hook --event="${EVENT}" --repo-root="${repo_root}"
