---
name: implementx
description: |
  Personal lifecycle — execute ALL waves in one breath. Main thread is pure orchestrator;
  all implementation work is strictly delegated to subagents. Coordinator visible content ≤35%.
  Sets drive_until_done true. REVIEW_GATE hard block off under lifecycle_profile interactive.
  Use for /implementx after /planx.
routing_layer: L0
routing_owner: owner
routing_gate: none
routing_gate_evidence: "ROADMAP.md + WAVE_STATE.json + lanes ≥3 unless small_task"
routing_priority: P1
session_start: n/a
user-invocable: true
trigger_hints:
  - /implementx
  - implementx
allowed_tools:
  - mcp__mcp-codegraph__codegraph_search
  - mcp__mcp-codegraph__codegraph_callers
  - mcp__mcp-codegraph__codegraph_callees
  - mcp__mcp-codegraph__codegraph_impact
  - mcp__mcp-codegraph__codegraph_node
  - mcp__mcp-codegraph__codegraph_status
  - mcp__mcp-codegraph__codegraph_goto_definition
metadata:
  version: "0.2.0"
  platforms: [supported]
  tags: [my-lifecycle, implement, multi-agent, one-breath, orchestrator-worker]
---

## Quick Ref
- **Purpose**: 产品交付 wave 执行器——主线程纯编排，所有实现严格委派给 subagent，一气呵成跑完所有 wave
- **Key Rules**: 主线程禁止写产品代码；parallel 每 wave ≥3 lane；subagent 必须写 lane-notes（五字段 schema）；one-breath 不停 wave 问用户
- **Trigger**: `/implementx`（通常接 `/planx` 之后）
<!-- full content below; load on demand -->

# implementx

（共享 header 见 [`../my-lifecycle-common/header.md`](../my-lifecycle-common/header.md)）

Under **`lifecycle_profile: interactive`**, Cursor **Stop** does **not** emit hard `router-rs AG_FOLLOWUP` (goal continuity is manual: `framework_goal_drive` stdio + `artifacts/current/<task_id>/` boards). **`beforeSubmit` does not arm `goal_required`** (uses `goal_drive_entry_active` for pre-goal only). Closeout / `CLOSEOUT_FOLLOWUP` may still apply when completion is claimed.

> **设计依据**：Anthropic *Building Effective Agents*（orchestrator-workers pattern）、*How we built our multi-agent research system*（3-5 parallel subagent、artifact system、incremental injection）、*Effective context engineering*（subagent isolation + 摘要回传）、*Scaling Managed Agents*（brain/hands/session 三层解耦）、Claude 官方 subagent/workflow/agent-teams 文档、GitHub maestro-orchestrate（426★）/ code-audit-system / ArtChiTech-framework 等实战案例。完整证据表见 [`references/orchestration-contract.md`](references/orchestration-contract.md)。

---

## 0. 核心身份（HARD）

**主线程 = 纯编排器（Orchestrator）。**

主线程 **禁止**：
- 直接编写、编辑、修改任何产品代码或配置文件
- 直接 Read 超过 50 行的源码文件（除非用于 wave 调度决策，如读 lane scope）
- 将 subagent 原始 transcript 全量回显到主聊天
- 代替 subagent 执行其 lane 范围内的工作
- 在 wave 中途跳过 lane 直接实现

主线程 **允许**：
- 读 `WAVE_STATE.json` / `ROADMAP.md` / `lane-notes/*.md` 做调度决策
- 跑 `verify_commands`（cargo test / npm test 等验收命令）
- 合并 lane-notes 为 ≤3 bullet 摘要
- 更新 `GOAL_STATE.json`（经 `framework_goal_drive` stdio）
- 在 subagent 完成后做集成验证

---

## 1. One-breath all-waves (HARD)

When invoked, run **every wave** in `WAVE_STATE.json` from current `wave_id` through the last wave **without** stopping at wave boundaries to ask the user.

| CAN continue (no user ping) | MUST stop |
|----------------------------|-----------|
| Next lane in parallel group | Scope/requirement error |
| Next wave after merge checkpoint | P0 security |
| Verification failed, fix obvious | External dependency down |
| Retry with new evidence | User said stop |

**Do not** treat "Wave N complete" as a pause point.

---

## 2. Lane Split-or-Explain (HARD)

| 条件 | 要求 |
|------|------|
| `execution_mode=parallel` | 每 wave **≥3 lane**（推荐 3-5）；lane 间 `scope_paths` 严格 disjoint |
| `execution_mode=serial` | 单 lane 允许，但 wave 之间必须串行有依赖链（`depends_on`） |
| 单 lane wave 且无依赖 | **必须**在 lane-notes 首行写 `reject_reason: small_task` 或说明为何不可拆 |
| planx 输出 <3 lane 且 parallel | implementx **拒绝执行**；提示用户回 `/planx` 重新拆分，或显式接受 `small_task` |

> **Coordinator 裁量（ADVISORY）**：当任务总文件数 ≤3 且总改动量 <100 行时，coordinator 可在 lane-notes 首行记录理由后降为 2 lane 执行，无需回退 planx。此为 ADVISORY 裁量。

**拆分启发式**（planx 参考）：
- 不同目录 / 模块 → 独立 lane
- 独立文件群（disjoint file set）→ 独立 lane
- read-heavy 探索 vs write 实现 → 独立 lane
- review / research vs implementation → 可并行 lane
- 单文件 > 100 行改动 → 拆为 ≥2 lane（接口 + 实现）

---

## 3. Subagent Dispatch Contract (HARD)

### 3.1 Handoff 模板（Anthropic 标准）

每个 lane spawn 时，prompt **必须**包含以下五元组：

```text
## Objective
<明确、可验证的目标，不超过 2 句>

## Scope
- Write scope: <scope_paths 列表，只写这些路径>
- Forbidden: 不得修改 <明确禁区>

## Output Format
- 写入文件：<scope_paths 内的文件>
- Lane notes: artifacts/current/<task_id>/lane-notes/<lane_id>.md
- Max 15 lines; 必须包含下方 schema 的全部字段

## Tools
- Allowed: Read, Write, Edit, Glob, Grep, Bash (仅限 scope_paths 内)
- Disallowed: 任何 scope_paths 外的 Write/Edit

## Verification
- 本地验证命令：<lane-specific verify command>
- 成功标准：<具体可判断的条件>
```

> **字段映射注释**：基于 Anthropic 博客五元组做了 implementx 适配——Scope 对应 `task_boundaries`，Verification 对应 `guidance`。完整溯源见 [`references/orchestration-contract.md`](references/orchestration-contract.md)。

### 3.2 Lane Return Schema (HARD)

每个 subagent **必须**在 `lane-notes/<lane_id>.md` 中写入以下字段（缺一不可）：

```markdown
# Lane: <lane_id>

## changed_files
- path/to/file1.rs: <one-line summary>
- path/to/file2.ts: <one-line summary>

## evidence
- <具体做了什么，关键 diff 摘要或命令输出>

## verification
- command: <实际运行的命令>
- result: pass | fail
- details: <失败时的错误摘要>

## risk
- <潜在风险或遗留问题，无则写 "none">

## next_action
- <下一步建议，无则写 "none">
```

**校验规则**：
- `changed_files` 为空且 scope_paths 含写操作 → fail-closed（lane 未完成）
- `verification.result=fail` → coordinator 决定 retry（最多 2 次）或 abort wave
- lane-notes 超过 20 行 → 截断警告（但不阻塞）

### 3.3 Capability Narrowing (HARD)

```json
{
  "lane_id": "w3-lane-auth",
  "scope_paths": ["core/host-projection/src/hosts/cursor_hooks/"],
  "output_path": "artifacts/current/<task_id>/lane-notes/w3-lane-auth.md",
  "max_lines": 15,
  "fork_context": false,
  "tools": {
    "allowed": ["Read", "Glob", "Grep", "Bash"],
    "disallowed": ["Write", "Edit"]
  },
  "forbidden": ["paste full transcript to main chat"]
}
```

| 约束 | 说明 |
|------|------|
| `fork_context` | **必须显式 `false`**（reviewer / researcher / implementation lane 均适用） |
| 写入 disjoint | 各 lane 仅写 `scope_paths` 内文件 |
| review 只读 | 默认 review-only；implementation lane 可写但仅限 scope_paths |
| Model inherit | **省略 `model` 参数**；继承主会话模型（Cursor / Codex 均适用） |
| Subagent 不能 spawn subagent | 单层 delegation（对齐 Claude 官方 subagent 规范） |

### 3.4 Token Budget by Role (ADVISORY)

| 角色 | 建议 token 上限 | 说明 |
|------|-----------------|------|
| researcher / explorer | 8,000 | 只读探索，回传摘要 |
| reviewer | 6,000 | findings-only，不写代码 |
| implementer | 15,000 | 写代码 + 本地验证 |
| verifier | 4,000 | 跑命令 + 判定 pass/fail |

coordinator **不得**将 subagent 全量 output 注入主 context；只读 lane-notes 摘要。

---

## 4. Async + Failure Cascade (HARD)

### 4.1 Spawn 超时

- 每个 lane spawn 设 **隐式超时**（由宿主 Task 工具控制）
- 超时 → 记录 `lane-notes/<lane>.timeout.md`，coordinator 决定 skip 或 retry

### 4.2 Failure Cascade 防御

| 场景 | 处理 |
|------|------|
| subagent 启动失败（region/model 不可用） | 写 `lane-notes/<lane>.fallback.md` 说明原因；coordinator 尝试主线程串行降级 |
| subagent 中途 crash | lane-notes 可能部分写入；coordinator 读已写部分决定 retry 或 skip |
| verify 命令不存在 | fail-closed；写 `lane-notes/<lane>.verify_missing.md` |
| lane 间依赖未满足 | wave 阻塞；不静默跳过 |

**禁止静默降级**：任何 fallback / skip **必须**写入 `lane-notes/<lane>.fallback.md`。

---

## 5. Main Thread Scheduler Protocol

1. **Read** `WAVE_STATE.json` + `ROADMAP.md`；校验 lane 数 ≥3（parallel）或有依赖链（serial）
2. **For each wave**（in order）：
   a. Spawn all lanes in `parallel_group`（`execution_mode=parallel`）或单 lane（serial）
   b. 等待所有 lane 完成（background spawn → TaskOutput 收集）
   c. **Merge**：Read `lane-notes/<lane>.md`（仅摘要）；主聊天 ≤3 bullet + paths
   d. 跑 `verify_commands`（如有）
   e. Update wave `status` → `completed`；`current_wave`++；checkpoint `EVIDENCE_INDEX`
3. After final wave → suggest `/verifyx`（or auto-chain if user asked full pipeline）

**Target**: coordinator visible content **≤35% of turn**。

> **操作性定义**：visible content 指 coordinator 在主聊天中输出的文字内容（不含工具调用 JSON 和 lane-notes 读取）。35% 为 advisory 指标，不硬拦。

---

## 6. CodeGraph 集成

以下高风险操作 **coordinator 必须在 spawn subagent 前**调用 codegraph 工具：

| 操作 | 必调工具 | 产出 |
|------|---------|------|
| 删除/重命名公共符号 | `codegraph_callers["符号名", depth=1]` | 确认无遗漏调用者 |
| 重构核心函数/类型 | `codegraph_impact["符号名", depth=2]` | 写入 lane scope 的调用链清单 |
| 跨模块修改 | `codegraph_callees["符号名", depth=2]` | 确认下游模块无破坏 |
| `scope_paths` 含 `core/` 或 `tools/` | `codegraph_impact["符号名", depth=3]` | 完整影响面报告 |

**subagent lane prompt 模板**中，在 Tools 段追加：
> 修改 `target_symbol` 前，调 `codegraph_impact["target_symbol"]` 获取影响半径，将结果写入 lane-notes。

> 详细场景见 [`codegraph-scenarios.md`](../shared-references/codegraph-scenarios.md)。

---

## 7. External Research Lane (OPTIONAL)

当 wave 包含 review / research 子任务时，可与 implementation 并行 spawn：

| Lane 类型 | 执行方式 | 产出 |
|-----------|---------|------|
| `implementation` | 写代码 + 本地验证 | `changed_files` + `lane-notes` |
| `review` | 只读审查（`fork_context=false`） | `findings` + `lane-notes` |
| `research` | 只读探索（`fork_context=false`） | `summary` + `lane-notes` |

**规则**：
- research / review lane **禁止写代码**
- coordinator **先 merge implementation**，再 merge research / review findings
- research 发现可触发 **incremental injection**：coordinator 将关键发现追加到后续 lane 的 prompt

---

## 8. Artifact System (HARD)

| 产物 | 写者 | 读者 |
|------|------|------|
| `scope_paths` 内的代码/配置 | subagent | coordinator（仅 verify_commands） |
| `lane-notes/<lane>.md` | subagent | coordinator |
| `EVIDENCE_INDEX` | coordinator | verifyx |
| `GOAL_STATE.json` | `framework_goal_drive` stdio | everyone（只读） |
| `WAVE_STATE.json` | coordinator（status 更新） | everyone（只读） |

**subagent 不直接相互通信**（对齐 Claude 官方规范）；所有跨 lane 信息经 coordinator 中转。

---

（共享 GOAL_STATE writes 段见 [`../my-lifecycle-common/header.md`](../my-lifecycle-common/header.md)）

## 9. GOAL_STATE on start

显式 stdio 启动（**无** Stop `GOAL_CONTINUE` hook 注入，2026-05 连续性拔除）。**`start` / `resume` 同时写入** `artifacts/current/active_task.json`（及默认 `focus_task.json`，`set_focus: false` 可跳过 focus）；**禁止**手改 `{}` 作指针占位。

```bash
# status=running, drive_until_done=true, lifecycle_profile=interactive
printf '%s\n' '{"id":1,"op":"framework_goal_drive","payload":{"operation":"start","repo_root":"<repo>","task_id":"<task_id>","goal":"<from GOAL_STATE>","drive_until_done":true,"status":"running","lifecycle_profile":"interactive"}}' | router-rs --stdio-json
```

`complete` / `clear` 会中性化指向该 `task_id` 的 active/focus 指针（删除文件，不留空对象）。

---

## 10. Cross-References

| 文档 | 用途 |
|------|------|
| [`skills/agent-swarm-orchestration/SKILL.md`](../agent-swarm-orchestration/SKILL.md) | spawn admission 判定 / reject reason |
| [`skills/planx/SKILL.md`](../planx/SKILL.md) | WAVE_STATE schema / lane 拆分规则 |
| [`skills/verifyx/SKILL.md`](../verifyx/SKILL.md) | 验证收口 |
| [`docs/hosts/hook-hosts.md` § 多代理编排](../../docs/hosts/hook-hosts.md) | Codex 端并行执行指引 |
| [`AGENTS.md`](../../AGENTS.md) | 完整执行阶梯规则（§ Coding First Principles） |
| [`references/orchestration-contract.md`](references/orchestration-contract.md) | 外部权威证据表 |

---

## 文档工程（Document Engineering）
当任务涉及项目文档（README、API docs、ADR、changelog、onboarding guide、docstrings）时，
读取 `references/doc-engineering-guide.md` 中的完整 checklist 和模板。

## 调试门控（Debug Gate）
根因未确认时，参考 `skills/systematic-debugging/SKILL.md` 的诊断协议。
不要在假设未验证的情况下跳到实现。

## CodeGraph 场景

Coordinator **只读**；subagent handoff 可引用图谱结论，但不替代 lane 内 Read/Grep。

> 工具与场景表：见 [`codegraph-scenarios.md`](../shared-references/codegraph-scenarios.md)。
> 何时：每 wave 调度前检查就绪（stale 时注明「先 sync/重建索引」）；拆 scope 前符号定位校验 `scope_paths`；并行 lane 确认 disjoint 与依赖方向；改公共 API 前评估影响半径。勿在 implement lane 内手改索引 DB。

---

## 11. Next

`/verifyx` — evidence + ship in one command.
