#!/usr/bin/env bash
# Verifies repo Cursor hooks wiring（无 review gate）：关键脚本存在 + postToolUse rust-lint 冒烟。
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

HOOKS_JSON="$REPO_ROOT/.cursor/hooks.json"

for path in \
  "$HOOKS_JSON" \
  "$REPO_ROOT/.cursor/hooks/resolve-repo-root.sh" \
  "$REPO_ROOT/.cursor/hooks/rust-lint.sh" \
  "$REPO_ROOT/.cursor/hooks/rustfmt.sh" \
  "$REPO_ROOT/.cursor/hooks/session-start.sh" \
  "$REPO_ROOT/.cursor/hooks/precompact-notice.sh"; do
  [[ -f "$path" ]] || {
    echo "verify_cursor_hooks: missing $path" >&2
    exit 1
  }
done

grep -q 'rust-lint.sh' "$HOOKS_JSON" || {
  echo "verify_cursor_hooks: .cursor/hooks.json must invoke rust-lint.sh (postToolUse)" >&2
  exit 1
}
grep -q 'session-start.sh' "$HOOKS_JSON" || {
  echo "verify_cursor_hooks: .cursor/hooks.json must invoke session-start.sh" >&2
  exit 1
}
grep -q 'rustfmt.sh' "$HOOKS_JSON" || {
  echo "verify_cursor_hooks: .cursor/hooks.json must invoke rustfmt.sh" >&2
  exit 1
}
grep -q 'precompact-notice.sh' "$HOOKS_JSON" || {
  echo "verify_cursor_hooks: .cursor/hooks.json must invoke precompact-notice.sh" >&2
  exit 1
}

if grep -q 'review-gate.sh' "$HOOKS_JSON"; then
  echo "verify_cursor_hooks: .cursor/hooks.json must not invoke review-gate.sh (gate disabled)" >&2
  exit 1
fi

printf '%s' '{"tool_name":"Write","tool_input":{"path":"/tmp/verify-no-rs.txt"}}' \
  | bash "$REPO_ROOT/.cursor/hooks/rust-lint.sh" >/dev/null || {
  echo "verify_cursor_hooks: rust-lint.sh smoke failed" >&2
  exit 1
}

echo "verify_cursor_hooks: ok"
