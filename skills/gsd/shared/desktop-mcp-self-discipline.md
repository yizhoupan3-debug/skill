---
name: gsd-desktop-mcp-self-discipline
description: |
  Desktop MCP self-discipline reminders for manual evidence recording.
  Required because Desktop MCP cannot auto-intercept commands or record evidence.
version: "1.0"
platforms: [desktop-mcp]
---

# Desktop MCP 自律模块

> ⚠️ **重要**: Desktop MCP 无法自动拦截命令或记录证据
> 所有验证后必须手动调用 `record_evidence`

## 自律检查清单

### 会话开始时

```markdown
┌─────────────────────────────────────────────────────────────┐
│ Desktop MCP 会话开始检查                                     │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│ 1. [ ] 调用 framework_digest 获取连续性摘要                │
│ 2. [ ] 检查 artifacts/current/<task_id>/ 状态文件          │
│ 3. [ ] 验证 GOAL_STATE.status                             │
│ 4. [ ] 检查 WAVE_STATE.current_wave                       │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

### 执行验证后

```markdown
┌─────────────────────────────────────────────────────────────┐
│ 验证后必须操作                                             │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│ 每次验证命令执行后:                                        │
│                                                             │
│ 调用 record_evidence:                                      │
│ ```                                                         │
│ record_evidence                                            │
│ command="<验证命令>"                                       │
│ result="pass|fail"                                        │
│ ```                                                         │
│                                                             │
│ 例如:                                                       │
│ record_evidence command="cargo test" result="pass"         │
│ record_evidence command="cargo clippy" result="pass"      │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

### 阶段性检查点

```markdown
┌─────────────────────────────────────────────────────────────┐
│ 阶段性检查点 (每完成一个 phase/wave)                        │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│ 1. 调用 session_checkpoint                                 │
│ 2. 更新 GOAL_STATE.json                                    │
│ 3. 更新 WAVE_STATE.json                                   │
│ 4. 更新 EVIDENCE_INDEX.json                               │
│ 5. 写入 SESSION_SUMMARY.md                                │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

### 会话结束时

```markdown
┌─────────────────────────────────────────────────────────────┐
│ Desktop MCP 会话结束检查                                    │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│ 1. [ ] 调用 closeout_gate 自检                            │
│ 2. [ ] 验证所有验证结果已记录                              │
│ 3. [ ] 更新最终 METRICS.json                              │
│ 4. [ ] 若完成，调用 goal_state_manage operation=complete   │
│ 5. [ ] 写入完整的 SESSION_SUMMARY.md                      │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

## 自律跳过追踪

如果跳过自律操作，会话结束后更新 METRICS.json：

```json
{
  "self_discipline": {
    "skipped_record_evidence": 0,
    "skipped_checkpoint": 0,
    "last_reminder_at": "ISO8601"
  }
}
```

## 软强制机制

| 操作 | CLI | Desktop MCP |
|------|-----|-------------|
| record_evidence | 自动 | **手动** (必须) |
| session_checkpoint | Stop hook | **手动** (建议) |
| closeout_gate | 可选 | **建议** |
| 危险命令拦截 | 自动 | 警告 |

## 快速参考

```bash
# 验证后记录证据
record_evidence command="cargo test" result="pass"
record_evidence command="cargo clippy" result="pass"

# 阶段完成时检查点
session_checkpoint
goal_state_manage operation=checkpoint

# 会话结束
closeout_gate
goal_state_manage operation=complete
```

## 常见陷阱

| 问题 | 原因 | 解决 |
|------|------|------|
| 忘记 record_evidence | 习惯 | 每次验证后立即调用 |
| EVIDENCE_INDEX 不完整 | 跨会话忘记 | 检查点时验证完整性 |
| 状态文件过时 | 未及时更新 | 每次关键操作后更新 |
