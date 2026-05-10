#!/usr/bin/env bash
# postToolUse hook — runs cargo check after Rust file writes and injects errors.
#
# PURPOSE: Code quality + token reduction. Catching compile errors immediately
# after a Write/StrReplace saves the "run check → read errors → fix → repeat"
# loop, which typically costs 3k–10k tokens per debugging round.
#
# Behaviour:
#   - Silent (returns {}) when the file isn't Rust or cargo check passes.
#   - Returns { additional_context } with error lines when check fails.
#   - Skips if cargo not found or check takes > 25 s（gtimeout / python3 subprocess 超时；否则依赖 hooks.json 总超时）.
#   - Best-effort: runs `router-rs framework hook-evidence-append` so continuity EVIDENCE_INDEX
#     records cargo check exit codes when the repo has continuity anchors (fail-open).
#
# INPUT:  postToolUse JSON (tool_name, tool_input.path, …)
# OUTPUT: { "additional_context": "…" }  OR  {}

set -uo pipefail

export PATH="$HOME/.cargo/bin:$PATH"
source "$HOME/.cargo/env" 2>/dev/null || true

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# gtimeout（brew coreutils）或 python3 subprocess.timeout；再无则直接运行（由 Cursor hook 总超时兜底）
_timeout_run() {
  local secs=$1
  shift
  if command -v gtimeout &>/dev/null; then
    gtimeout "$secs" "$@"
    return $?
  fi
  if command -v python3 &>/dev/null; then
    python3 - "$secs" "$@" <<'PY'
import subprocess
import sys

secs = float(sys.argv[1])
cmd = sys.argv[2:]
try:
    p = subprocess.run(cmd, timeout=secs)
    raise SystemExit(p.returncode)
except subprocess.TimeoutExpired:
    sys.stderr.write("rust-lint: cargo check exceeded timeout\n")
    raise SystemExit(124)
PY
    return $?
  fi
  "$@"
  return $?
}

emit_additional_context() {
  local ctx="$1"
  if command -v jq &>/dev/null; then
    jq -n --arg ctx "$ctx" '{additional_context: $ctx}'
  else
    python3 -c 'import json,sys; print(json.dumps({"additional_context": sys.argv[1]}))' "$ctx"
  fi
}

PASS='{}'; TIMEOUT_S=25; MAX_ERROR_LINES=20

input=$(cat)
tool_name=$(echo "$input" | jq -r '.tool_name // empty' 2>/dev/null)
file_path=$(echo "$input" | jq -r '.tool_input.path // empty' 2>/dev/null)

# Only act on file-writing tools that touch .rs files
case "$tool_name" in
    Write|StrReplace|write|str_replace) ;;
    *) echo "$PASS"; exit 0 ;;
esac

[[ "$file_path" == *.rs ]] || { echo "$PASS"; exit 0; }
[[ -f "$file_path" ]]      || { echo "$PASS"; exit 0; }

command -v cargo &>/dev/null || { echo "$PASS"; exit 0; }

# Walk up to find Cargo.toml
cargo_dir=""
dir=$(dirname "$file_path")
while [[ "$dir" != "/" && "$dir" != "." ]]; do
    [[ -f "$dir/Cargo.toml" ]] && { cargo_dir="$dir"; break; }
    dir=$(dirname "$dir")
done
[[ -n "$cargo_dir" ]] || { echo "$PASS"; exit 0; }

# Run cargo check — no link step, fast feedback
output=$(cd "$cargo_dir" && _timeout_run "$TIMEOUT_S" cargo check --message-format=short 2>&1)
rc=$?

# Continuity: append cargo check outcome to artifacts/current/EVIDENCE_INDEX.json when router-rs exists
# (no-op if continuity not seeded). Same env as Codex: ROUTER_RS_CONTINUITY_POSTTOOL_EVIDENCE=0 disables.
REPO_ROOT="$(cd "$cargo_dir" && "${SCRIPT_DIR}/resolve-repo-root.sh")"
ROUTER_BIN=""
if [[ -n "$REPO_ROOT" ]]; then
  ROUTER_BIN="$(bash "${SCRIPT_DIR}/resolve-router-rs.sh" "$REPO_ROOT")"
fi
if [[ -z "$ROUTER_BIN" ]]; then
  ROUTER_BIN="$(command -v router-rs 2>/dev/null || true)"
fi
if [[ -n "$REPO_ROOT" && -x "$ROUTER_BIN" ]]; then
  CMD_PREVIEW="$(printf '(cd %s && cargo check --message-format=short)' "$cargo_dir")"
  if command -v jq &>/dev/null; then
    JSON=$(jq -n \
      --arg root "$REPO_ROOT" \
      --arg cmd "$CMD_PREVIEW" \
      --argjson rc "$rc" \
      --arg src "cursor_rust_lint" \
      '{repo_root:$root, command_preview:$cmd, exit_code:$rc, source:$src}')
  else
    JSON="$(
      PG_ROOT="$REPO_ROOT" PG_CMD="$CMD_PREVIEW" PG_RC="$rc" python3 <<'PY'
import json
import os

print(
    json.dumps(
        {
            "repo_root": os.environ["PG_ROOT"],
            "command_preview": os.environ["PG_CMD"],
            "exit_code": int(os.environ["PG_RC"]),
            "source": "cursor_rust_lint",
        }
    )
)
PY
    )"
  fi
  "$ROUTER_BIN" framework hook-evidence-append --input-json "$JSON" >/dev/null 2>&1 || true
fi

[[ $rc -eq 0 ]] && { echo "$PASS"; exit 0; }

if [[ $rc -eq 124 ]]; then
  basename=$(basename "$file_path")
  emit_additional_context "cargo check timed out after ${TIMEOUT_S}s while checking ${basename} (crate: ${cargo_dir}). Consider running cargo check manually or raising rust-lint TIMEOUT_S."
  exit 0
fi

# Extract error lines; fall back to last N lines if none found
errors=$(echo "$output" | grep -E "^error(\[E[0-9]+\])?" | head -"$MAX_ERROR_LINES")
[[ -z "$errors" ]] && errors=$(echo "$output" | tail -"$MAX_ERROR_LINES")

basename=$(basename "$file_path")
msg="cargo check failed after editing ${basename}:
${errors}

Fix these errors before finalizing. Run \`cargo check\` to verify."

emit_additional_context "$msg"
