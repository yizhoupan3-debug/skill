#!/usr/bin/env bash
# 供其它 hook 脚本 source；勿单独执行。
#
# 与 router-rs `cursor_hook_silent_by_env` 对齐：ROUTER_RS_CURSOR_HOOK_SILENT 为真 → 静默（少日志/少注入）。

router_rs_cursor_hook_silent() {
  local t
  t="${ROUTER_RS_CURSOR_HOOK_SILENT:-}"
  t="$(printf '%s' "$t" | sed -e 's/^[[:space:]]*//' -e 's/[[:space:]]*$//' | tr '[:upper:]' '[:lower:]')"
  case "$t" in
    ''|0|false|off|no) return 1 ;;
    *) return 0 ;;
  esac
}
