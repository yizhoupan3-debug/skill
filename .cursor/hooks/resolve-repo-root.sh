#!/usr/bin/env bash
# 解析「含 .cursor/hooks.json 的仓库根」，供 Cursor hooks 在 git 不可用或 cwd 在子目录时仍指向策略真源。
# 优先级：CURSOR_WORKSPACE_ROOT → ROUTER_RS_CURSOR_WORKSPACE_ROOT → git toplevel → pwd。
# 输出：单行绝对路径（无换行后缀，便于 $(...) 捕获）。

set -uo pipefail

find_hooks_root() {
  local start="$1"
  local dir
  dir="$(cd "$start" 2>/dev/null && pwd)" || return 1
  local i=0
  while [[ "$dir" != "/" && i -lt 40 ]]; do
    if [[ -f "$dir/.cursor/hooks.json" ]]; then
      printf '%s' "$dir"
      return 0
    fi
    dir="$(dirname "$dir")"
    i=$((i + 1))
  done
  return 1
}

candidates=()
[[ -n "${CURSOR_WORKSPACE_ROOT:-}" ]] && candidates+=("$CURSOR_WORKSPACE_ROOT")
[[ -n "${ROUTER_RS_CURSOR_WORKSPACE_ROOT:-}" ]] && candidates+=("$ROUTER_RS_CURSOR_WORKSPACE_ROOT")
if root="$(git rev-parse --show-toplevel 2>/dev/null)"; then
  candidates+=("$root")
fi
candidates+=("$(pwd)")

for c in "${candidates[@]}"; do
  [[ -z "$c" ]] && continue
  if r="$(find_hooks_root "$c" 2>/dev/null)"; then
    [[ -n "$r" ]] && printf '%s' "$r" && exit 0
  fi
done

printf '%s' "$(pwd)"
