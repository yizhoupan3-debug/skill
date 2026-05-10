#!/usr/bin/env bash
# Verifies repo Cursor hooks wiring：关键脚本存在、review-gate/autopilot 门接线、postToolUse 冒烟。
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

HOOKS_JSON="$REPO_ROOT/.cursor/hooks.json"
HARNESS_NUDGES="$REPO_ROOT/configs/framework/HARNESS_OPERATOR_NUDGES.json"

for path in \
  "$HOOKS_JSON" \
  "$HARNESS_NUDGES" \
  "$REPO_ROOT/.cursor/hooks/resolve-repo-root.sh" \
  "$REPO_ROOT/.cursor/hooks/resolve-router-rs.sh" \
  "$REPO_ROOT/.cursor/hooks/review-gate.sh" \
  "$REPO_ROOT/.cursor/hooks/post-tool-use.sh" \
  "$REPO_ROOT/.cursor/hooks/rust-lint.sh" \
  "$REPO_ROOT/.cursor/hooks/rustfmt.sh" \
  "$REPO_ROOT/.cursor/hooks/session-start.sh" \
  "$REPO_ROOT/.cursor/hooks/precompact-notice.sh" \
  "$REPO_ROOT/.cursor/hooks/precompact-full.sh"; do
  [[ -f "$path" ]] || {
    echo "verify_cursor_hooks: missing $path" >&2
    exit 1
  }
done

python3 - "$HOOKS_JSON" <<'PY'
import json
import sys

path = sys.argv[1]
with open(path, encoding="utf-8") as f:
    payload = json.load(f)

hooks = payload.get("hooks")
if not isinstance(hooks, dict):
    raise SystemExit("verify_cursor_hooks: .cursor/hooks.json must contain a hooks object")

def commands(event):
    entries = hooks.get(event)
    if not isinstance(entries, list) or not entries:
        raise SystemExit(f"verify_cursor_hooks: missing hook event {event}")
    out = []
    for entry in entries:
        command = entry.get("command") if isinstance(entry, dict) else None
        if isinstance(command, str):
            out.append(command)
    if not out:
        raise SystemExit(f"verify_cursor_hooks: event {event} must contain command hooks")
    return out

def require(event, needle):
    if not any(needle in command for command in commands(event)):
        raise SystemExit(f"verify_cursor_hooks: {event} must invoke {needle}")

for event in ("beforeSubmitPrompt", "stop", "subagentStart", "subagentStop"):
    require(event, "review-gate.sh")
require("sessionEnd", "review-gate.sh")
require("postToolUse", "post-tool-use.sh")
require("sessionStart", "session-start.sh")
require("afterFileEdit", "rustfmt.sh")
require("preCompact", "precompact-full.sh")
PY

printf '%s' '{"tool_name":"Write","tool_input":{"path":"/tmp/verify-no-rs.txt"}}' \
  | bash "$REPO_ROOT/.cursor/hooks/rust-lint.sh" >/dev/null || {
  echo "verify_cursor_hooks: rust-lint.sh smoke failed" >&2
  exit 1
}

echo "verify_cursor_hooks: ok"
