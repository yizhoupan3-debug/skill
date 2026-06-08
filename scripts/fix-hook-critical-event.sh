#!/usr/bin/env bash
# 一键修复 claude-router-rs-hook.sh 的 critical_event block 行为
# 修复内容：router-rs 不可用时统一 allow，与 CLAUDE.md "无 shell 硬拦" 一致
set -euo pipefail

ROOT="${CLAUDE_PROJECT_ROOT:-$(git rev-parse --show-toplevel 2>/dev/null || pwd)}"
HOOK="$ROOT/configs/framework/claude-router-rs-hook.sh"

if [[ ! -f "$HOOK" ]]; then
  echo "ERROR: $HOOK not found"; exit 1
fi

# 备份
BACKUP="${HOOK}.bak.$(date +%s)"
cp "$HOOK" "$BACKUP"
echo "✓ backup → $BACKUP"

# Python 替换：删除 3 行（if critical_event / printf block / exit 1 / fi）
python3 - "$HOOK" << 'PYEOF'
import sys, pathlib

p = pathlib.Path(sys.argv[1])
lines = p.read_text().splitlines(keepends=True)

# 要删除的行特征
out = []
skip_depth = 0
for line in lines:
    stripped = line.strip()
    
    if skip_depth == 0:
        if stripped.startswith('if ') and 'critical_event "$HOOK_EVENT"' in stripped:
            skip_depth = 1
            continue
        out.append(line)
    else:
        # We are inside the if-block to remove
        if stripped.startswith('if '):
            skip_depth += 1
        elif stripped == 'fi' or stripped.startswith('fi '):
            skip_depth -= 1
        continue


p.write_text(''.join(out))
print(f"✓ removed critical_event block ({len(lines) - len(out)} lines)")
PYEOF

# 验证
echo ""
echo "=== verify ==="
if grep -q 'critical_event "$HOOK_EVENT"' "$HOOK"; then
  echo "FAIL: critical_event block still present"
  exit 1
fi
echo "✓ critical_event block removed"

if grep -q '"decision":"allow"' "$HOOK"; then
  echo "✓ allow fallback preserved"
else
  echo "WARN: allow decision not found"
fi

echo ""
echo "=== diff ==="
diff "$BACKUP" "$HOOK" || true

echo ""
echo "=== result context ==="
grep -n -A2 -B1 'ROUTER_RS_BIN' "$HOOK" | tail -8
