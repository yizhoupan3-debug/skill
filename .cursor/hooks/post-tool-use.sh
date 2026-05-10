#!/usr/bin/env bash
# postToolUse：只读一次 stdin。先跑 review-gate（更新 pre_goal / phase），再跑 rust-lint（对外输出）。
# 若 hooks.json 里并列两个命令而宿主不重放 stdin，第二个 hook 会收到空载荷，导致 pre_goal 永不满足。
# PostToolUse 非零退出（含 ROUTER_RS_CURSOR_HOOK_STRICT=1 / router 缺失 fail-closed）会中断脚本，不再跑 rust-lint，避免「门控失败仍当成功」。
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
INPUT="$(cat)"

printf '%s' "$INPUT" | bash "$SCRIPT_DIR/review-gate.sh" PostToolUse >/dev/null
printf '%s' "$INPUT" | bash "$SCRIPT_DIR/rust-lint.sh"
