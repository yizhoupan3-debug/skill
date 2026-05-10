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

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=hook-common.sh
source "${SCRIPT_DIR}/hook-common.sh"

EVENT="${1:?review-gate.sh: event name required (e.g. BeforeSubmitPrompt)}"
INPUT="$(cat)"
repo_root="$("${SCRIPT_DIR}/resolve-repo-root.sh")"

ROUTER_BIN="$(bash "${SCRIPT_DIR}/resolve-router-rs.sh" "$repo_root")"

# router-rs 未构建时无法按 session_key 精确删文件；SessionEnd 仍应清空 hook-state，避免下一会话继承 RG_FOLLOWUP。
cursor_hook_state_cleanup_on_session_end_fallback() {
  local root="$1"
  local d="${root}/.cursor/hook-state"
  [[ -d "$d" ]] || return 0
  find "$d" -mindepth 1 -maxdepth 1 -exec rm -rf {} + 2>/dev/null || true
}

if [[ -z "${ROUTER_BIN}" || ! -x "${ROUTER_BIN}" ]]; then
  event_lc="$(printf '%s' "$EVENT" | tr '[:upper:]' '[:lower:]')"
  if [[ "$event_lc" == "sessionend" ]]; then
    cursor_hook_state_cleanup_on_session_end_fallback "$repo_root"
  fi
  msg="review-gate: router-rs binary not found (see .cargo/config.toml target-dir + scripts/router-rs); gate disabled. Run: (cd scripts/router-rs && cargo build --release)"
  if ! router_rs_cursor_hook_silent; then
    echo "$msg" >&2
  fi
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
