---
name: gsd-metrics-protocol
description: |
  Token efficiency and execution metrics collection protocol.
  Tracks: RTK savings, completion rate, context usage, subagent parallelization
version: "1.0"
platforms: [desktop-mcp, cli, codex, cursor]
---

# GSD Metrics Protocol

## Overview

GSD Metrics tracks execution efficiency across all GSD commands for continuous improvement.

## Key Metrics

| Metric | Calculation | Target | Alert |
|--------|-------------|--------|-------|
| Token Efficiency | `saved_tokens / total_tokens` | ≥60% | <40% |
| Command Completion | `completed / total` | ≥90% | <70% |
| Verification Pass | `verified / total` | 100% | <90% |
| RFV Health | `avg_rounds` | 2-3 | <1 or >5 |
| Subagent Parallelism | `avg_parallel` | ≥3 | <2 |
| Context Usage | `used / limit` | ≤50% | >70% |
| Resume Success | `resumed / total` | ≥95% | <80% |

## METRICS.json Schema

```json
{
  "schema_version": "gsd-metrics-v1",
  "task_id": "string",
  "session_id": "uuid",
  "created_at": "ISO8601",
  "commands": [
    {
      "command_id": "gsd-execute-phase-1",
      "command": "/gsd-execute-phase",
      "started_at": "ISO8601",
      "completed_at": "ISO8601|null",
      "status": "running|completed|failed",
      "tokens": {
        "input_start": 1000,
        "input_end": 1500
      },
      "rtk_savings": {
        "saved_tokens": 200,
        "savings_percent": 25
      },
      "subagent_count": 3,
      "parallel_degree": 2.5,
      "evidence_count": 5
    }
  ],
  "aggregates": {
    "total_commands": 10,
    "completed_commands": 9,
    "total_tokens_saved": 2000,
    "avg_rtk_savings_percent": 30
  }
}
```

## Collection Points

### 1. Command Start
```bash
# Record start metrics
jq ".commands += [{
  \"command_id\": \"$(date +%Y%m%d-%H%M%S)\",
  \"command\": \"$1\",
  \"started_at\": \"$(date -u +%Y-%m-%dT%H:%M:%SZ)\",
  \"status\": \"running\"
}]" METRICS.json > METRICS.tmp && mv METRICS.tmp METRICS.json
```

### 2. Command Complete
```bash
# Update completion metrics
jq "(.commands[] | select(.command_id == \"$CMD_ID\") | .status) = \"completed\"
    | (.commands[] | select(.command_id == \"$CMD_ID\") | .completed_at) = \"$(date -u +%Y-%m-%dT%H:%M:%SZ)\"
    | .aggregates.total_commands += 1
    | .aggregates.completed_commands += 1" METRICS.json > METRICS.tmp && mv METRICS.tmp METRICS.json
```

### 3. RTK Integration
```bash
# Get RTK savings
rtk gain >> ~/.gsd/audit.log
```

## METRICS.md Human-Readable Summary

```markdown
# GSD 执行指标

## 当前会话
- 命令: /gsd-execute-phase
- 开始: 2026-05-19 10:00 UTC
- Token 使用: 1,500 → 2,200 (+700)
- RTK 节省: 200 tokens (28%)

## 累积统计
| 指标 | 本次 | 累计 |
|------|------|------|
| 完成命令 | 1 | 5/6 |
| 验证通过 | 3/3 | 8/8 |
| Token 节省 | 28% | 32% |
```

## Context Usage Thresholds

| Usage | Level | Action |
|-------|-------|--------|
| <50% | Healthy | Normal operation |
| 50-60% | Caution | Log warning |
| 60-70% | Warning | Checkpoint recommended |
| 70-80% | Critical | Save immediately |
| >80% | Urgent | Spawn fresh subagent |

## RTK Integration

```bash
# ~/.gsd/audit.log format
[2026-05-19 10:00:00] /gsd-execute-phase: input=1500 output=2200 saved=300 pct=25
[2026-05-19 10:15:00] /gsd-verify-work: input=2200 output=2800 saved=400 pct=30
```

## Analytics

```bash
# View token efficiency trend
cat ~/.gsd/audit.log | awk '{print $NF}' | sort -n | uniq -c

# Calculate average savings
cat ~/.gsd/audit.log | grep -o 'pct=[0-9]*' | sed 's/pct=//' | awk '{sum+=$1; cnt++} END {print sum/cnt "%"}'
```
