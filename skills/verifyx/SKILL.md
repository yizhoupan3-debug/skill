---
name: verifyx
description: |
  Personal lifecycle — verify + ship in one command. Evidence index, tests, closeout, goal complete.
  Use after /implementx. Merges legacy verify-work and ship checklists.
routing_layer: L0
routing_owner: owner
routing_gate: none
routing_gate_evidence: "WAVE_STATE.json global_status=completed"
routing_priority: P1
session_start: n/a
user-invocable: true
trigger_hints:
  - /verifyx
  - verifyx
metadata:
  version: "0.2.0"
  platforms: [supported]
  tags: [my-lifecycle, verify, ship, evidence]
---

# verifyx

**Zone**: execution+ · **profile**: `my-light` · **no separate ship command**

## CLI flags

| Flag | Description |
|------|-------------|
| `--no-purge` | 跳过 post-verify 任务目录清理（lane-notes 及全部 `artifacts/current/<task_id>/`）。closeout record 内嵌 evidence 不受影响。 |

用法示例：

```bash
/verifyx --no-purge        # 验证 + closeout，但保留 task-dir
/verifyx                   # 验证 + closeout + 延迟清理（默认行为）
```

## Checklist (single pass)

### 1. Verify

- Run `GOAL_STATE.validation_commands` and ROADMAP global verification commands
- Append each run to `EVIDENCE_INDEX.json` `artifacts[]` rows (`command_preview`, `recorded_at`, `exit_code`, `success`, optional `kind` / `lifecycle_command` / `tags`)
- `VERIFY_REPORT.md` summary on disk

### 2. Ship

- Git clean / intentional uncommitted documented
- `closeout_evaluate` stdio（或 CLI `router-rs closeout evaluate --record-path artifacts/closeout/<task_id>.json`）→ 写入 closeout record（**embed** evidence rows / verify summary before purge）
- `framework_goal_drive` → `operation: complete` — 遵循 [../my-lifecycle-common/GOAL_STATE_CONTRACT.md](../my-lifecycle-common/GOAL_STATE_CONTRACT.md) 中的 GOAL_STATE 写入规范
- Closeout `notes` may record purge intent (e.g. `task_artifacts_purged; task_dir_removed`) — **not** separate schema fields; `CLOSEOUT_RECORD_SCHEMA.json` / `CloseoutRecord` use `deny_unknown_fields`

### 3. Post-verify task-dir purge (**deferred by default**)

> **`--no-purge` 传入时**：跳过本节所有操作，task-dir 保留在 `artifacts/current/<task_id>/`。closeout record 内嵌的 evidence 行不受影响。

**Order**: closeout JSON written → then mark or delete.

**默认行为（无 `--no-purge`）：延迟清理**

closeout 写入后，不立即删除 task-dir，而是在 `artifacts/current/<task_id>/` 内写入一个 `.purge-after` 标记文件，内容为 ISO-8601 时间戳（= 当前时间 + 24 小时）：

```bash
TASK_ID=<task_id>
# After closeout evaluate succeeded:
date -u -v+24H +%Y-%m-%dT%H:%M:%SZ > "artifacts/current/${TASK_ID}/.purge-after"
```

下次 `/verifyx` 启动时（步骤 1 之前），扫描所有 `artifacts/current/*/` 中的 `.purge-after` 文件：
- 若时间戳已过期 → 执行 `rm -rf` 清除该 task-dir 并删除 `.purge-after`
- 若时间戳未过期 → 跳过

此机制保证 closeout record 写入后仍保留 24 小时回溯窗口，同时在下次 verify 时自动回收。

**显式立即清理**（与旧行为等价）：若需在本次 verify 内立即删除，手动执行：

```bash
TASK_ID=<task_id>
rm -rf "artifacts/current/${TASK_ID}"
```

移除 `artifacts/current/<task_id>/` 下全部 My 生命周期产物（`REQUIREMENTS.md`、`DECISIONS.md`、`OPEN_QUESTIONS.md`、`ROADMAP.md`、`WAVE_STATE.json`、`GOAL_STATE.json`、`EVIDENCE_INDEX.json`、`VERIFY_REPORT.md`、`lane-notes/`、`SCHEMA_DRIFT_BASELINE.json`、`.purge-after`）。

**Only ship artifact**: `artifacts/closeout/<task_id>.json`（closeout record 内已 embed evidence rows / verify summary）。

Pointer cleanup：遵循 [../my-lifecycle-common/GOAL_STATE_CONTRACT.md](../my-lifecycle-common/GOAL_STATE_CONTRACT.md) 中的指针管理规范；其余 `task_registry.json`、`.supervisor_state.json` 按宿主文档手工中性化。

### 4. Chat

≤5 lines: PASS/FAIL, closeout path, purge status (purged / deferred-24h / skipped-by-flag). No command dumps.

## Pre-conditions

- `WAVE_STATE.global_status` = `completed` (or implement waived with user ack)

## Canonical evidence protocol

See `skills/verifyx/references/evidence-protocol.md`.

## Schema drift

Headings contract: `configs/framework/SCHEMA_DRIFT_HEADINGS_CONTRACT.md`. Run `schema-drift baseline` + `check` **before** writing `.purge-after` marker (or before immediate delete if manually purging).
