#!/usr/bin/env bash
set -euo pipefail

ROOT="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
CODEX_HOME="${CODEX_HOME:-$HOME/.codex}"

mkdir -p "$CODEX_HOME"

CONFIG="$CODEX_HOME/config.toml"
HOOKS="$CODEX_HOME/hooks.json"

if [[ ! -f "$CONFIG" ]]; then
  printf '%s\n' '[features]' 'codex_hooks = true' '' > "$CONFIG"
elif ! rg -q '^\s*codex_hooks\s*=\s*true\s*$' "$CONFIG"; then
  if rg -q '^\s*\[features\]\s*$' "$CONFIG"; then
    # Append inside existing [features] block if present; otherwise append a new block.
    printf '\n%s\n%s\n' '[features]' 'codex_hooks = true' >> "$CONFIG"
  else
    printf '\n%s\n%s\n' '[features]' 'codex_hooks = true' >> "$CONFIG"
  fi
fi

cat > "$HOOKS" <<'JSON'
{
  "hooks": {
    "UserPromptSubmit": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "/usr/bin/env python3 \"$(git rev-parse --show-toplevel)/.codex/hooks/review_subagent_gate.py\"",
            "timeout": 10,
            "statusMessage": "Checking review/subagent gate"
          }
        ]
      }
    ],
    "PostToolUse": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "/usr/bin/env python3 \"$(git rev-parse --show-toplevel)/.codex/hooks/review_subagent_gate.py\"",
            "timeout": 10,
            "statusMessage": "Updating review/subagent gate state"
          }
        ]
      }
    ],
    "Stop": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "/usr/bin/env python3 \"$(git rev-parse --show-toplevel)/.codex/hooks/review_subagent_gate.py\"",
            "timeout": 10,
            "statusMessage": "Enforcing review/subagent gate"
          }
        ]
      }
    ]
  }
}
JSON

chmod 0644 "$HOOKS" || true

printf 'Installed codex-cli hooks into %s\n- %s\n- %s\n' "$CODEX_HOME" "$CONFIG" "$HOOKS"

