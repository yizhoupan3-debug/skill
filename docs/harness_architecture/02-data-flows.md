---
last_verified: "2026-06-02"
depends_on:
  - ../host_adapter_contract.md
  - ../references/AGENTS_OPERATOR_SURFACE.md
  - ../task_state_unified_resolve.md
  - ../rfv_loop_harness.md
---

# 热路径真源与数据流

[返回索引](index.md)

## 2. 热路径真源

### 2.1 SessionStart（2026-05 连续性拔除后）

- Codex / Cursor SessionStart **不**注入连续性 digest、`GOAL_CONTINUE` / `RFV_LOOP_CONTINUE` 或 `depth_compliance` / `depth_compliance_refresh_hint` 段落（2026-05 hook 路径已拔除；`depth_compliance_aggregate` 与 `depth_compliance_refresh_hint` 仍供 **stdio / `task-state-resolve` / 遗留 env 单测**，**不**注入 SessionStart）。
- **`ROUTER_RS_OPERATOR_INJECT` 总闸**：闸关时 Codex 无 `additionalContext`、Cursor `additional_context` 为空；闸开时仅允许 **轻量** 动态信息：Cursor **`Repo:`** 单行（[`handle_session_start`](../../core/router-rs/src/hosts/cursor_hooks/handlers_parts/handlers_session.inc.rs)）；Codex `SessionStart source:`。**无** digest、**无** SessionStart 指针 hint（分裂观测用 `framework task-state-resolve` / `framework doctor`）。
- **禁止**：repo onboarding、Quick Reference、Build & test、Key paths、Tool cost hierarchy 等静态说明；禁止恢复 hook 驱动的 `GOAL_CONTINUE` / `RFV_LOOP_CONTINUE`。
- 出站仍按 UTF-8 **字节**预算截断（Cursor `...[~trunc]`；Codex `...`）。
- 宏目标 / RFV 多轮：仅 **`framework_goal_drive` / `framework_rfv_loop` stdio** 与 `artifacts/current/<task_id>/` 手动画板；见 [`AGENTS_OPERATOR_SURFACE.md`](../references/AGENTS_OPERATOR_SURFACE.md)。

### 2.2 Skill routing

- `skills/SKILL_ROUTING_RUNTIME.json` 是唯一**热路由**真源；运行时由 `core/router-rs/src/route/records.rs` 机读。
- 热 runtime 只保留：`version`、`schema_version`、`scope`、`keys`、`skills`。
- 任何 plugin、projection、routing explain、兼容迁移叙事都不进热 runtime。
- 冷真源 = **编译器 / 契约 / CI 消费集**，并非 hook 热路径读物：
  - [`skills/SKILL_PLUGIN_CATALOG.json`](../../skills/SKILL_PLUGIN_CATALOG.json)：`router-rs framework skills` 校验/刷新；policy contract 消费。
  - [`skills/SKILL_ROUTING_METADATA.json`](../../skills/SKILL_ROUTING_METADATA.json)：路由 metadata 真源；`tests/policy_contracts.rs` 与 `host_integration/mod.rs` 校验。
  - [`skills/SKILL_ROUTING_RUNTIME_EXPLAIN.json`](../../skills/SKILL_ROUTING_RUNTIME_EXPLAIN.json)：路由解释器衍生物，policy 契约校验目标；不要把它当 router-rs 第二真源去删。

### 2.3 控制面配置与生成物（2026-05-20 硬化）

| 真源 | 用途 |
|------|------|
| [`configs/framework/RUNTIME_REGISTRY.json`](../../configs/framework/RUNTIME_REGISTRY.json) | 闭集宿主、`review_gate.reviewer_lanes`、profile 投影；**lane 缓存**经 [`core-policy/registry_review_gate.rs`](../../core/core-policy/src/registry_review_gate.rs)（`runtime_registry` re-export + nudge 文案）。读盘失败时 lane 判定 **fail-closed**；`framework doctor` 探测 snapshot。改 lane 后重启 hook 子进程即可，**无需** `cargo build`。 |
| [`configs/framework/host_projection_narrative.json`](../../configs/framework/host_projection_narrative.json) | 各宿主 framework 投影内的 **My lifecycle 默认链** 与 **review findings-only** 英文段落；`framework host-integration install` 渲染时读取。叙事政策仍以 [`AGENTS.md`](../../AGENTS.md) 为跨宿主真源，本 JSON 仅为安装产物文案真源。 |
| [`configs/framework/GENERATED_ARTIFACTS.json`](../../configs/framework/GENERATED_ARTIFACTS.json) | 声明须纳入版本库的生成物路径、generator 命令与 `compare` 模式（`byte-for-byte` / `normalized-text`）。 |

**`generated-artifacts-status` 两种模式**（[`host_integration/mod.rs`](../../core/router-rs/src/host_integration/mod.rs)）：

| 模式 | 触发 | 行为 |
|------|------|------|
| **metadata-only** | `framework doctor`（默认）；CLI `--skip-generator-run`；env `ROUTER_RS_GENERATED_ARTIFACTS_SKIP_GENERATORS=1` | 不跑 manifest generator；检查声明路径存在、forbidden marker、undeclared 路径与 per-artifact `clean`；`manifest_status.mode` = `manifest-backed-generated-artifact-metadata-only`。 |
| **drift-gate（全量）** | `framework maint update-one-shot`；显式全量探针 | 在隔离 temp root 执行声明 generator（含 `host-integration install` 等慢步骤，默认单 generator **300s** 超时，可用 `ROUTER_RS_GENERATOR_TIMEOUT_SECONDS` 覆盖），再 byte/normalized 对比 checked-in 与再生副本。 |

集成测试与日常 `doctor` 应使用 **metadata-only**；提交前维护流仍须至少一次 **drift-gate** 绿（见 [`skills/update/SKILL.md`](../../skills/update/SKILL.md)）。

**Cursor project L4 手维护面**（相对 Codex/Claude 的 `GENERATED_ARTIFACTS` drift-gate **不对称**；`framework host-integration install --to cursor` **不**托管 project hooks）：

| 路径 | 说明 |
|------|------|
| `.cursor/hooks.json` | 7 事件闭集；parity 见 `scripts/ci/check-cursor-hooks-parity.sh` |
| `.cursor/router-rs-hook.env` | hook 子进程 env |
| `.cursor/rules/*.mdc` | gate/plan alwaysApply rules |
| `.cursor/commands/*.md` | Framework slash stubs（7 文件：`discussx` / `planx` / `implementx` / `verifyx` / `gitx` / `update` / `deepinterview`） |
| `.cursor/agents/deep-reviewer.md` | 深度 review lane 定义 |
| `configs/framework/cursor-router-rs-hook.sh` | launcher（repo 根相对路径经 hooks.json command 引用） |

User-scope install 产出：`~/.cursor/rules/framework.mdc`（叙事真源 `host_projection_narrative.json`）。详见 [`docs/hosts/cursor.md`](../hosts/cursor.md)。

**Companion 生成物**（`SKILL_PLUGIN_CATALOG.json`、`SKILL_HEALTH_MANIFEST.json` 等）：`source_of_truth: false` stub；默认 `cargo test --test policy_contracts` 只断言闭集与形态，**不**恢复历史 `capability_classes` 富契约（见 `plugin_catalog_routing_metadata_and_health_manifest_form_closed_loop`，`tests/policy_contracts.rs`）。

## 3. 主数据流

### 3.1 证据流

`L1` 验证命令或验证形工具输出
→ `router-rs` 采样/追加
→ `artifacts/current/<task_id>/EVIDENCE_INDEX.json`
→ closeout / gate 消费。

原则：

- hook 只记录证据，不替模型"编造验证通过"。
- 长尾命令通过显式 append 或更窄启发式补齐。

**Task ledger 写入（跨进程）**：`GOAL_STATE` / `RFV_LOOP_STATE` / `STEP_LEDGER.jsonl` append（`framework step-ledger`）、session artifact 批量写 / `EVIDENCE_INDEX` 的 RMW，默认经 [`task_write_lock.rs`](../../core/antigravity/src/utils/task_write_lock.rs) 在 `artifacts/current/.router-rs.task-ledger.lock` 上持 **`flock(2)`**，按**仓库根**互斥（多宿主 hook 子进程不共享 Rust 进程内互斥量）。`EVIDENCE_INDEX` 仍可再持单文件旁路锁（`runtime_storage::acquire_runtime_path_lock`）；锁序约定为 **先 repo ledger flock，再 path lock**。`runtime_storage` 的 **memory** 回归后端对 `append_text` 使用进程内 `Mutex` 串行化（不参与 repo flock）。`ROUTER_RS_TASK_LEDGER_FLOCK=0|false|off|no` 可关闭 flock（如不稳定的网络 FS），关闭后并行写为 best-effort；`router-rs framework doctor` 在 flock 关闭时会打印醒目提示。`TASK_STATE.json` 仅为投影，权威仍以分文件为准；聚合失败会以 stderr 前缀 **`TASK_STATE_AGGREGATE_SYNC_FAILED`** 记录；单独跑 `task-state-aggregate-sync` 的修复路径不替代上述写锁。

### 3.1.1 轨迹与 step 恢复流

`TRACE_EVENTS.jsonl` 是轨迹诊断流，复用 `trace_runtime record-event`，用于记录
`task_id / owner / gate / overlay / horizon / phase / tool_or_lane / status /
failure_class / evidence_ref / context_bytes` 等复盘字段。它不替代
`EVIDENCE_INDEX`：前者解释过程，后者支撑验证。

`STEP_LEDGER.jsonl` 是 task-scoped 长任务 step 恢复流，由
`router-rs framework step-ledger` 追加；`TASK_STATE.json` 只投影摘要
（条数、状态计数、最新 step/ref），不把整份 ledger 注入模型上下文。

统一 failure taxonomy 的机器可读表在
`configs/framework/HARNESS_FAILURE_TAXONOMY.json`；behavioral eval fixture 表在
`configs/framework/HARNESS_BEHAVIORAL_EVAL_CASES.json`。这些配置只描述评估与分类，
不成为第二套路由、证据或 closeout 真源。

### 3.2 续跑与门控流

磁盘状态
→ `router-rs`
→ 宿主输出字段
→ 模型可见提示。

固定投影策略：

- 硬门控短码进 `followup_message`
- advisory 提示进 `additional_context`

**2026-05**：hook **不**再投影 `GOAL_CONTINUE` / `RFV_LOOP_CONTINUE`；续跑仅 stdio + 手动画板（见 §2.1）。

**可读模型**：当 `active_task.json` 指向的任务缺少可读 `GOAL_STATE.json`，但 `focus_task.json` 指向另一任务且该任务盘上存在合法 GOAL 时，[`resolve_task_view`](../../core/antigravity/src/task_state.rs) 会在 `resolution_notes` 写入短码 `continuity:active_goal_missing_focus_has_goal`（仅观测；[`read_goal_state_for_hydration`](../../core/router-rs/src/autopilot_goal.rs) 仍不回退 focus）。`framework task-state-resolve` 可透出该行提示。

**Stop 自动 checkpoint（已删除，2026-05）**：Cursor/Codex hook **不再**在 Stop 写盘。显式 checkpoint 仅用 Desktop MCP `session_checkpoint` 或 `framework_session_artifact_write` stdio。

**L1 运行时视图与读模型**：[`load_framework_runtime_view`](../../core/router-rs/src/framework_runtime/runtime_view.rs) 的 `active_task_id` 选择与 [`resolve_task_view`](../../core/antigravity/src/task_state.rs) 一致（`override > active > focus > supervisor`），见 [`task_state_unified_resolve.md`](../task_state_unified_resolve.md)。
