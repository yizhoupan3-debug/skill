#!/usr/bin/env bash
# preCompact：保留用量提示（precompact-notice）并合并 router-rs PreCompact 输出
#（review gate 阶段摘要 + RFV 一行 hint）。router-rs 缺失时 fail-open，仅输出用量 JSON。
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
INPUT="$(cat)"

token_json="$(printf '%s' "$INPUT" | bash "$SCRIPT_DIR/precompact-notice.sh")"
router_json="$(printf '%s' "$INPUT" | bash "$SCRIPT_DIR/review-gate.sh" PreCompact)"

printf '%s\n%s\n' "$token_json" "$router_json" | jq -s 'reduce .[] as $o ({}; . * $o)'
