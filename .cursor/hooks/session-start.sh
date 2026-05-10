#!/usr/bin/env bash
# sessionStart hook — injects a compact project briefing into session context.
#
# PURPOSE: Token reduction. Gives the agent key facts (build cmds, paths,
# conventions) in ~300 tokens at session start, preventing first-turn file
# reads of README, AGENTS.md, skill manifests (typically 5k–15k tokens saved).
#
# Continuity: when Codex/router-rs has written artifacts/current/SESSION_SUMMARY.md,
# prepend an excerpt so Cursor sessions can resume context without a manual Read.
#
# OUTPUT: additional_context (injected into system context for the session)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$("${SCRIPT_DIR}/resolve-repo-root.sh")"
SUMMARY_PATH="$PROJECT_ROOT/artifacts/current/SESSION_SUMMARY.md"
CONTINUITY_BLOCK=""
if [[ -f "$SUMMARY_PATH" ]]; then
  CONTINUITY_BLOCK="$(printf '\n\n## Continuity (artifacts/current/SESSION_SUMMARY.md)\n%s\n' "$(head -n 36 "$SUMMARY_PATH")")"
fi

read -r -d '' CONTEXT << EOF || true
## Skill Repo — Quick Reference

**Root:** $PROJECT_ROOT
**Stack:** Rust (scripts/router-rs/), bash scripts, TOML/JSON config

**Build & test:**
- \`cd scripts/router-rs && cargo build --release\`
- \`cd scripts/router-rs && cargo test\`
- \`cd scripts/router-rs && cargo clippy -- -D warnings\`

**Key paths:**
- \`scripts/router-rs/src/\` — Rust hook router (cursor_hooks.rs, route.rs, …)
- \`skills/SKILL_ROUTING_RUNTIME.json\` — skill routing truth source (use this, not skills/ dir)
- \`.cursor/hooks.json\` — Cursor hook config
- \`AGENTS.md\` — agent execution policy
- \`artifacts/current/\` — continuity checkpoints (SESSION_SUMMARY, NEXT_ACTIONS, …)

**Conventions:**
- Rust: clippy-clean, rustfmt-formatted, no bare \`unwrap()\` in library paths
- Skills: always route via SKILL_ROUTING_RUNTIME.json; never pre-read the full skills/ dir
- JSON: 2-space indent
- Git commits: only when user explicitly asks

**Tool cost hierarchy (cheapest first):**
Shell → Glob → Grep → Read → StrReplace/Write → SemanticSearch → MCP
${CONTINUITY_BLOCK}
EOF

jq -n --arg ctx "$CONTEXT" '{"additional_context": $ctx}'
