#!/usr/bin/env bash
# postToolUse — combines rust compile check (rust-lint.sh) with review gate
# state updates (router-rs PostToolUse). Single hook avoids Cursor merging
# ambiguity when multiple postToolUse hooks return additional_context.
#
# Merge uses jq when available; otherwise python3 (same semantics).
# stdin: Cursor postToolUse JSON
# stdout: merged JSON for Cursor

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$("${SCRIPT_DIR}/resolve-repo-root.sh")"
INPUT="$(cat)"

LINT_JSON="$(echo "$INPUT" | "${ROOT}/.cursor/hooks/rust-lint.sh")"
GATE_JSON="$(echo "$INPUT" | "${ROOT}/.cursor/hooks/review-gate.sh" PostToolUse)"

merge_with_jq() {
  jq -n \
    --argjson lint "${LINT_JSON:-"{}"}" \
    --argjson gate "${GATE_JSON:-"{}"}" '
    ($lint.additional_context // "") as $lc |
    ($gate.additional_context // "") as $gc |
    ($lint | del(.additional_context)) as $lrest |
    ($gate | del(.additional_context)) as $grest |
    ($lrest + $grest) as $merged |
    if ($lc != "" and $gc != "") then
      $merged + {additional_context: ($lc + "\n\n" + $gc)}
    elif ($lc != "") then
      $merged + {additional_context: $lc}
    elif ($gc != "") then
      $merged + {additional_context: $gc}
    else
      $merged
    end
  '
}

merge_with_python() {
  PG_LINT_JSON="$LINT_JSON" PG_GATE_JSON="$GATE_JSON" python3 <<'PY'
import json
import os

lint = json.loads(os.environ["PG_LINT_JSON"])
gate = json.loads(os.environ["PG_GATE_JSON"])
lc = lint.get("additional_context") or ""
gc = gate.get("additional_context") or ""
lrest = {k: v for k, v in lint.items() if k != "additional_context"}
grest = {k: v for k, v in gate.items() if k != "additional_context"}
merged = {**lrest, **grest}
if lc and gc:
    merged["additional_context"] = lc + "\n\n" + gc
elif lc:
    merged["additional_context"] = lc
elif gc:
    merged["additional_context"] = gc
print(json.dumps(merged, separators=(",", ":")))
PY
}

if command -v jq &>/dev/null; then
  merge_with_jq
elif command -v python3 &>/dev/null; then
  merge_with_python
else
  # 无法合并时优先保留 gate 侧状态（router-rs），lint 反馈丢失优于 gate 静默失效
  printf '%s\n' "${GATE_JSON:-{}}"
fi
