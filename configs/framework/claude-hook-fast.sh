#!/usr/bin/env bash
# Fast-path hook launcher for Claude Code.
# Replaces hook.sh for Claude: sources env + exec binary directly.
# ~60-70ms vs ~100ms for full hook.sh path.
# Fail-open: if binary missing or errors, Claude treats as pass-through.
set -uo pipefail

ROOT="${CLAUDE_PROJECT_ROOT:-$PWD}"
FW="${SKILL_FRAMEWORK_ROOT:-$ROOT}"
ENV_FILE="$ROOT/.claude/router-rs-hook.env"
if [ -r "$ENV_FILE" ]; then
  set -a; . "$ENV_FILE"; set +a
fi
BIN="${ROUTER_RS_BIN:-router-rs-cli}"
if ! command -v "$BIN" >/dev/null 2>&1; then
  for p in "$HOME/.local/bin/router-rs-cli" "$FW/target/release/router-rs-cli" \
           "$HOME/.local/bin/router-rs" "$FW/target/release/router-rs"; do
    if [ -x "$p" ]; then BIN="$p"; break; fi
  done
fi
exec "$BIN" host hook --event="$1" --repo-root "$ROOT" claude
