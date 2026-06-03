---
last_verified: "2026-06-02"
depends_on:
  - harness_architecture/index.md
  - operator_profiles.md
---

# Harness policy map（叙事裁判地图）

本文件回答：**某一类策略「以谁为真源」**、Cursor `.mdc` / skill 应扮演什么角色。目标是减法：避免在规则碎片里维护第二份总纲。

| 主题 | Canonical（裁判） | 只读派生 / 宿主差异 |
|------|-------------------|---------------------|
| 跨宿主语言、执行梯子、review 清门叙事、Git 边界、Knowledge hygiene | 仓库根 [`AGENTS.md`](../AGENTS.md)（对应章节见下表） | Cursor alwaysApply [`.cursor/rules/*.mdc`](../.cursor/rules/) **只写宿主硬差异**；须含指向 [`harness_policy_map.md`](harness_policy_map.md)、[`AGENTS.md`](../AGENTS.md)、[`harness_architecture.md`](harness_architecture/index.md) 的 markdown 链接。**CI**（[`tests/policy_cursor_rules_links.rs`](../tests/policy_cursor_rules_links.rs)）**仅**校验：`alwaysApply: true` 且首段 frontmatter 可被该测试解析闭合的 `.mdc`；目录内其它 `.mdc` 不在此项内。CI 还约定：链接须形如 `](url)` 且 `url` 满足相对指针形（`../`、`./`、`docs/`、或路径段中的 `/docs/`）；枚举规则文件时 `read_dir` **只扫 `.cursor/rules/` 一层**（不递归子目录）；**正文不要写独占行 `alwaysApply:`**（避免「opening `---` 后缺闭合 `---`」的畸形分支误判）。 |
| 五层模型、证据流、续跑/门控、`ROUTER_RS_*` 语义与默认值 | [`harness_architecture/`](harness_architecture/index.md)（尤其 **[§5 开关面](harness_architecture/03-hook-and-switches.md#5-开关面)**） | [`router_env_flags.rs`](../core/router-rs/src/router_env_flags.rs) 仅提供 **helper 子集** + 注释索引；散落 `std::env::var` 仍以 harness §5 为准 |
| Skill 命中路径与 trigger | [`skills/SKILL_ROUTING_RUNTIME.json`](../skills/SKILL_ROUTING_RUNTIME.json) 的 `skill_path` | 各 `skills/**/SKILL.md` **不得**顶替 `AGENTS.md` 的总协议；命中后只读该 skill，不把 skill 全文当「第二 AGENTS」 |
| 验证命令成功/失败（机读） | [`artifacts/current/<task_id>/EVIDENCE_INDEX.json`](../artifacts/current/) | 聊天复述、Plan todo 勾选**不能**单独充当 ship 证据；`GOAL_STATE` + `EVIDENCE_INDEX` 为 Cursor Stop `AG_FOLLOWUP` 磁盘真源（`ship_readiness.rs`）；「验证通过」等词**不**再触发 closeout 与 goal verify 双拦 |
| Cursor Stop 门控编排 | [`ship_readiness.rs`](../core/router-rs/src/ship_readiness.rs) + [`handlers_stop.inc.rs`](../core/router-rs/src/hosts/cursor_hooks/handlers_parts/handlers_stop.inc.rs) | 每轮 Stop **最多一条** hard `followup_message`：closeout → REVIEW_GATE → AG_FOLLOWUP（`primary_fix=`）；`my-light` 仅 closeout + 软提示 |
| 深度 review 产出形状（compact envelope、lane） | [`skills/code-review-deep/SKILL.md`](../skills/code-review-deep/SKILL.md) | `AGENTS.md` Execution Ladder 只指向该 skill，不复制 lens 表 |
| Cursor 子代理模型继承 | [`.cursor/rules/subagent-model-inherit.mdc`](../.cursor/rules/subagent-model-inherit.mdc) + registry `subagent_model_inherit_nudge` | [`docs/hosts/cursor.md`](hosts/cursor.md)；`ROUTER_RS_CURSOR_SUBAGENT_MODEL_INHERIT_NUDGE` 见 harness §5 |
| Cursor Plan / CreatePlan 契约 | [`skills/plan-mode/SKILL.md`](../skills/plan-mode/SKILL.md) | [`.cursor/rules/cursor-plan-output.mdc`](../.cursor/rules/cursor-plan-output.mdc) 保留 CreatePlan 硬自检条（宿主工具差异） |
| 运维「低噪声 / solo」开关组合 | 仍以 harness §5 **逐变量**为裁判 | [`operator_profiles.md`](operator_profiles.md) 仅给 **可复制 profile**，默认值以 harness 表为准 |

## 与 `AGENTS.md` 章节锚点（读总协议时打开）

| 主题 | `AGENTS.md` 节 |
|------|----------------|
| 语言（简体中文默认） | [Language](../AGENTS.md#language) |
| Subagent / My 执行区 / 拒因 token | [Execution Ladder](../AGENTS.md#execution-ladder) |
| Review 默认与 skill 路由 | 同上节 + [Skill Routing](../AGENTS.md#skill-routing) |
| Closeout / 证据 | [Closeout](../AGENTS.md#closeout) |
| 连续性手动画板 | [打开 `AGENTS.md`](../AGENTS.md)，页内搜索 `## Continuity artifacts（手动画板 only）`（避免各渲染器中文锚点不一致）；续跑仅 `framework_goal_drive` / `framework_rfv_loop` stdio + `artifacts/current/<task_id>/` |
| 改哪里才生效（权威分层表） | [打开 `AGENTS.md`](../AGENTS.md)，页内搜索 `## 权威分层（改哪里才生效）`（同上） |

## `ROUTER_RS_*` 与 hook 行为

- **语义与默认**：只认 [`harness_architecture/03-hook-and-switches.md §5`](harness_architecture/03-hook-and-switches.md#5-开关面) 表格及该节正文脚注（含「`_CHARS` 实为字节」等命名债说明）。
- **实现入口**：行为在 `router-rs` 各模块；环境变量读取入口索引见 [`router_env_flags.rs`](../core/router-rs/src/router_env_flags.rs) 模块头注释。

## Skills 边界（防「第二 AGENTS」）

- 路由：只读 [`SKILL_ROUTING_RUNTIME.json`](../skills/SKILL_ROUTING_RUNTIME.json)。
- Skill 文件承载 **工作流与产出形状**；跨宿主不变量（语言、梯子、清门）仍以 **`AGENTS.md` + harness** 为准。

## 后续根本重构候选（不在当前 harness 减法交付内）

以下条目属于 **下一阶段 execution** 或独立 RFC；默认不改动 steady-state 行为。

1. **`REVIEW_GATE` 默认路径简化** — **已交付（2026-05-28）**：`ROUTER_RS_CURSOR_REVIEW_GATE_MODE=lite` + [`docs/adr/ADR-review-gate-lite.md`](adr/ADR-review-gate-lite.md) + [`configs/framework/CURSOR_SUBAGENT_HOOK_CONTRACT.json`](../configs/framework/CURSOR_SUBAGENT_HOOK_CONTRACT.json)；strict 仍为默认。后续候选：stdin 版本化后 lite 升为默认（见 task `review-gate-optimize-20260528` O1 抽样门禁）。
2. **账本合并（`EVIDENCE_INDEX` / `TRACE_EVENTS` / `STEP_LEDGER`）**：单一 append 流可降低锁与排障成本；**风险**：破坏现有工具与 `framework snapshot` 消费方；**前置**：读模型迁移、`TASK_STATE` 聚合语义冻结；需独立 RFC（2026-05 已删旧草案）。
3. **多宿主 hook 生成式减少分叉**：把 lane/`fork_context` 归一等逻辑更多 codegen 到单源；**风险**：生成器 bug 影响三宿主；**前置**：`host_integration` 契约测试覆盖阈值明确。
4. **`framework_profile` 大表与日常 solo 解耦**：发布面字段与自用 hook 最小面拆分；**风险**：profile 校验与安装路径分裂；**前置**：明确「发行 profile」与「repo dev profile」两个 ID。
5. **`ROUTER_RS_*` 命名统一（`_CHARS` → 字节）**：减少误读；**风险**：外部脚本依赖旧名；**前置**：别名读取窗口 + 弃用日志（若保留兼容期）。
6. **Codex embed（`AGENTS.md` + `AGENTS_CODEX.md`）与 sync 材料化（`AGENTS_CODEX.md`）漂移**：继续依赖 rebuild + `framework sync-entrypoints` / `codex sync`；**不建议**默认改为每次 hook 读盘（性能与确定性 trade-off）；可选加强 CI 对比 compile-time embed 哈希。

每条均需单独计划、测试矩阵与回滚策略；**不要**与本「叙事地图 + profile + 漂移哨兵」交付混为单次 PR。
