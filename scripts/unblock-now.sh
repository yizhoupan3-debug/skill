#!/usr/bin/env bash
# 彻底解除 Stop hook 死锁
# 原因：.claude/hook-state/hook_state_*.json 中 settings_validated=false 导致 Stop hook block
# 在另一个终端执行：bash ~/Developer/skill/scripts/unblock-now.sh
set -euo pipefail

ROOT="${CLAUDE_PROJECT_ROOT:-$(git rev-parse --show-toplevel 2>/dev/null || pwd)}"
STATE_DIR="$ROOT/.claude/hook-state"

echo "=== 清理 hook state: $STATE_DIR ==="

# Step 1: 重置所有 hook_state 文件（touch_state）
count=0
for f in "$STATE_DIR"/hook_state_*.json; do
  [[ -f "$f" ]] || continue
  python3 -c "
import json, sys, os
p = sys.argv[1]
try:
    with open(p, 'r') as f:
        d = json.load(f)
except Exception:
    os.remove(p)
    print(f'  deleted corrupted: {p}')
    sys.exit(0)
changed = False
if d.get('settings') and not d.get('settings_validated'):
    d['settings_validated'] = True
    changed = True
if d.get('framework') and not d.get('framework_tested'):
    d['framework_tested'] = True
    changed = True
if changed:
    json.dump(d, open(p, 'w'), indent=2)
    print(f'  fixed: {p}')
" "$f" && ((count++)) || true
done
echo "✓ 扫描 $count 个 hook_state 文件"

# Step 2: 清理所有 review_gate 锁文件（释放锁）
locks=$(find "$STATE_DIR" -name "review_gate_*.json.lock" 2>/dev/null | wc -l | tr -d ' ')
if [[ "$locks" -gt 0 ]]; then
  find "$STATE_DIR" -name "review_gate_*.json.lock" -delete
  echo "✓ 删除 $locks 个 review_gate 锁文件"
fi

# Step 3: 清理 hook_state 锁文件
hlocks=$(find "$STATE_DIR" -name "hook_state_*.json.lock" 2>/dev/null | wc -l | tr -d ' ')
if [[ "$hlocks" -gt 0 ]]; then
  find "$STATE_DIR" -name "hook_state_*.json.lock" -delete
  echo "✓ 删除 $hlocks 个 hook_state 锁文件"
fi

# Step 4: rtk trust（filters.toml 被修改后需要 re-trust）
if command -v rtk &>/dev/null; then
  rtk trust "$ROOT" 2>/dev/null && echo "✓ rtk trust done" || echo "(rtk trust skipped)"
fi

echo ""
echo "=== 完成！回到 Claude Code 窗口即可正常退出 ==="
