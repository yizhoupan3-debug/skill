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
# Long-task: if active_task.json points at a task dir containing GOAL_STATE.json /
# RFV_LOOP_STATE.json (written by router-rs stdio ops), inject compact snapshots so
# sessions resume even when beforeSubmit/stop are NOT wired to router-rs cursor hook.
#
# OUTPUT: additional_context (injected into system context for the session)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=hook-common.sh
source "${SCRIPT_DIR}/hook-common.sh"
PROJECT_ROOT="$("${SCRIPT_DIR}/resolve-repo-root.sh")"
SUMMARY_PATH="$PROJECT_ROOT/artifacts/current/SESSION_SUMMARY.md"
CONTINUITY_BLOCK=""
if [[ -f "$SUMMARY_PATH" ]]; then
  CONTINUITY_BLOCK="$(printf '\n\n## Continuity (artifacts/current/SESSION_SUMMARY.md)\n%s\n' "$(head -n 36 "$SUMMARY_PATH")")"
fi

LONG_TASK_BLOCK=""
if ! router_rs_cursor_hook_silent; then
  ACTIVE_TASK_JSON="$PROJECT_ROOT/artifacts/current/active_task.json"
  if [[ -f "$ACTIVE_TASK_JSON" ]] && command -v jq >/dev/null 2>&1; then
    TID="$(jq -r '.task_id // empty' "$ACTIVE_TASK_JSON" 2>/dev/null || true)"
    if [[ -n "${TID:-}" && "$TID" != "null" ]]; then
      GS="$PROJECT_ROOT/artifacts/current/$TID/GOAL_STATE.json"
      RV="$PROJECT_ROOT/artifacts/current/$TID/RFV_LOOP_STATE.json"
      GBLK=""
      RVBLK=""
      if [[ -f "$GS" ]]; then
        GBLK="$(jq -r --arg tid "$TID" '
          def trunc(s; n):
            if (s | type) == "string" and (s | length) > n then s[0:n-3] + "..." else s end;
          "## Goal（`artifacts/current/" + $tid + "/GOAL_STATE.json`）\n" +
          "- " + (.status // "?") + " · drive=" + (.drive_until_done | tostring) + " · "
            + (trunc(.goal // "-"; 120)) + "\n"
        ' "$GS" 2>/dev/null || true)"
      fi
      if [[ -f "$RV" ]]; then
        RVBLK="$(jq -r --arg tid "$TID" '
          def trunc(s; n):
            if (s | type) == "string" and (s | length) > n then s[0:n-3] + "..." else s end;
          "## RFV（`artifacts/current/" + $tid + "/RFV_LOOP_STATE.json`）\n" +
          "- " + (.loop_status // "?") + " · round " + ((.current_round // 0) | tostring)
            + "/" + ((.max_rounds // 0) | tostring) + " · " + (trunc(.goal // "-"; 80)) + "\n"
        ' "$RV" 2>/dev/null || true)"
      fi
      if [[ -n "$GBLK" || -n "$RVBLK" ]]; then
        LONG_TASK_BLOCK="$(printf '\n%s\n%s' "${GBLK:-}" "${RVBLK:-}")"
      fi
    fi
  fi
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
${CONTINUITY_BLOCK}${LONG_TASK_BLOCK}
EOF

jq -n --arg ctx "$CONTEXT" '{"additional_context": $ctx}'
