#!/usr/bin/env bash
set -euo pipefail

CODEX_HOME="${CODEX_HOME:-$HOME/.codex}"
SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)"
REPO_ROOT="$(cd -- "$SCRIPT_DIR/.." && pwd -P)"
HOOK_SCRIPT="$REPO_ROOT/.codex/hooks/review_subagent_gate.py"
HOOK_COMMAND="/usr/bin/env python3 \"$HOOK_SCRIPT\""

mkdir -p "$CODEX_HOME"

CONFIG="$CODEX_HOME/config.toml"
HOOKS="$CODEX_HOME/hooks.json"

if ! command -v python3 >/dev/null 2>&1; then
  printf 'python3 is required but was not found in PATH\n' >&2
  exit 1
fi

if [[ ! -f "$HOOK_SCRIPT" ]]; then
  printf 'Hook script not found: %s\n' "$HOOK_SCRIPT" >&2
  exit 1
fi

if [[ ! -r "$HOOK_SCRIPT" ]]; then
  printf 'Hook script is not readable: %s\n' "$HOOK_SCRIPT" >&2
  exit 1
fi

if [[ ! -f "$CONFIG" ]]; then
  printf '%s\n' '[features]' 'codex_hooks = true' '' > "$CONFIG"
else
  python3 - "$CONFIG" <<'PY'
import pathlib
import sys

path = pathlib.Path(sys.argv[1])
text = path.read_text(encoding="utf-8")

lines = text.splitlines()
out = []
in_features = False
features_seen = False
codex_hooks_set = False

for line in lines:
    stripped = line.strip()
    if stripped.startswith("[") and stripped.endswith("]"):
        if in_features and not codex_hooks_set:
            out.append("codex_hooks = true")
            codex_hooks_set = True
        in_features = stripped == "[features]"
        features_seen = features_seen or in_features
        out.append(line)
        continue

    if in_features and stripped.startswith("codex_hooks"):
        out.append("codex_hooks = true")
        codex_hooks_set = True
    else:
        out.append(line)

if in_features and not codex_hooks_set:
    out.append("codex_hooks = true")
    codex_hooks_set = True

if not features_seen:
    if out and out[-1].strip():
        out.append("")
    out.extend(["[features]", "codex_hooks = true"])

text = "\n".join(out).rstrip() + "\n"

path.write_text(text, encoding="utf-8")
PY
fi

HOOKS_EXISTED=0
BACKUP=""
if [[ -f "$HOOKS" ]]; then
  HOOKS_EXISTED=1
  BACKUP="$HOOKS.bak.$(date +%Y%m%d%H%M%S)"
  cp "$HOOKS" "$BACKUP"
fi

if ! python3 - "$HOOKS" "$HOOK_COMMAND" <<'PY'
import json
import pathlib
import sys

hooks_path = pathlib.Path(sys.argv[1])
hook_command = sys.argv[2]

if hooks_path.exists():
    data = json.loads(hooks_path.read_text(encoding="utf-8"))
    if not isinstance(data, dict):
        raise SystemExit(f"Invalid hooks.json root type: {type(data).__name__}")
else:
    data = {}

hooks_root = data.setdefault("hooks", {})
if not isinstance(hooks_root, dict):
    raise SystemExit("Invalid hooks.json: `hooks` must be an object")

def ensure_event_hook(event_name: str, message: str):
    event_entries = hooks_root.setdefault(event_name, [])
    if not isinstance(event_entries, list):
        raise SystemExit(f"Invalid hooks.json: hooks.{event_name} must be an array")

    for entry in event_entries:
        if not isinstance(entry, dict):
            continue
        nested = entry.get("hooks")
        if not isinstance(nested, list):
            continue
        for hook in nested:
            if not isinstance(hook, dict):
                continue
            if hook.get("type") == "command" and hook.get("command") == hook_command:
                return

    event_entries.append({
        "hooks": [{
            "type": "command",
            "command": hook_command,
            "timeout": 10,
            "statusMessage": message,
        }]
    })

ensure_event_hook("UserPromptSubmit", "Checking review/subagent gate")
ensure_event_hook("PostToolUse", "Updating review/subagent gate state")
ensure_event_hook("Stop", "Enforcing review/subagent gate")

tmp_path = hooks_path.with_suffix(hooks_path.suffix + ".tmp")
tmp_path.write_text(json.dumps(data, indent=2, ensure_ascii=True) + "\n", encoding="utf-8")
tmp_path.replace(hooks_path)
PY
then
  if [[ -n "$BACKUP" && -f "$BACKUP" ]]; then
    cp "$BACKUP" "$HOOKS"
  fi
  printf 'Failed to update hooks file: %s\n' "$HOOKS" >&2
  exit 1
fi

if [[ $HOOKS_EXISTED -eq 0 ]]; then
  chmod 0644 "$HOOKS" || true
fi

printf 'Installed codex-cli hooks into %s\n- %s\n- %s\n' "$CODEX_HOME" "$CONFIG" "$HOOKS"

