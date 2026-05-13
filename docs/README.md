# 文档索引（控制面与契约）

**叙事分工**：仓库根 `AGENTS.md` = 跨宿主执行与语言策略；[`harness_architecture.md`](harness_architecture.md) = 连续性 **L1–L5 控制面**上层真源；[`rust_contracts.md`](rust_contracts.md)（英文）= `router-rs` 实现侧契约长文；[`history/`](history/) = 迁移与旧方案归档，**不作为当前运行真源**。

## 推荐阅读顺序

1. [仓库根 README.md](../README.md) — 分享、安装、Cursor/Codex hook 快速入门  
2. [framework_operator_primer.md](framework_operator_primer.md) — 使用者一页纸：宿主差异、`REVIEW_GATE` 快查、真源阅读顺序、自检 `framework doctor`  
3. [AGENTS.md](../AGENTS.md) — Skill 路由、Continuity、Closeout、Execution Ladder  
4. [harness_architecture.md](harness_architecture.md) — 五层模型、证据流、续跑流、扩展规则（含 `HARNESS_OPERATOR_NUDGES`）  
5. [rust_contracts.md](rust_contracts.md) — 路由、profile、宿主集成、EVIDENCE_INDEX 等 Rust 业主  
6. [task_state_unified_resolve.md](task_state_unified_resolve.md) — `ResolvedTaskView` / `framework task-state-resolve`  

## 按主题

| 主题 | 文档 |
|------|------|
| 使用者视角：宿主差异、门控快查、阅读顺序 | [framework_operator_primer.md](framework_operator_primer.md) |
| RFV 多轮账本（`framework_rfv_loop`）契约与 lane 模板；数理推理强度 | [rfv_loop_harness.md](rfv_loop_harness.md)，[references/rfv-loop/](references/rfv-loop/)（含 [math-reasoning-harness.md](references/rfv-loop/math-reasoning-harness.md)）；**ADR**：[`close_gates` 与 `max_rounds` 收口路径](plans/ADR_rfv_close_gates_max_rounds.md) |
| 弱模型 / 上下文预算、Token 注入路径与 harness 合成交付 | [RESEARCH_harness_weak_model_top_tier.md](plans/RESEARCH_harness_weak_model_top_tier.md)，[context_token_audit_deep_dive.md](plans/context_token_audit_deep_dive.md)；全盘减法审计勾选表（与 PR 短清单互补） [harness_subtraction_first_principles_audit_checklist.md](plans/harness_subtraction_first_principles_audit_checklist.md) |
| Closeout 程序化门禁与 schema | [closeout_enforcement.md](closeout_enforcement.md)，`configs/framework/CLOSEOUT_RECORD_SCHEMA.json` |
| `framework_profile` 与默认面 | [framework_profile_contract.md](framework_profile_contract.md) |
| 新宿主接入 / 多宿主适配 | [§3.1 工程清单](host_adapter_contract.md#31-可复制执行清单工程顺序)（文首 **快速路径** 同文件）；`RUNTIME_REGISTRY`、`hook_common`、`review_gate` 与 Codex/Cursor/Claude 投影边界统一见 [host_adapter_contract.md](host_adapter_contract.md)；Round 2 计划镜像 [plans/harness_host_round2.md](plans/harness_host_round2.md)；Harness 改进 backlog（路线图展开） [plans/harness_improvement_backlog.md](plans/harness_improvement_backlog.md) |
| Cursor Plan / 可验收 todo | [`skills/plan-mode/SKILL.md`](../skills/plan-mode/SKILL.md)（含轻量 / execution / audit plan、**`plan_profile: research`**、**CreatePlan 输出契约**、Git 状态证据收口、**调研范围与能力联动**）；[`.cursor/rules/cursor-plan-output.mdc`](../.cursor/rules/cursor-plan-output.mdc)；短清单 [plans/plan_todo_checklist.md](plans/plan_todo_checklist.md)；能力调研合成 [plans/plan_writing_capability_research_synthesis.md](plans/plan_writing_capability_research_synthesis.md)；草稿目录 `.cursor/plans/` |
| Codex 宿主投影边界 | [host_adapter_contract.md](host_adapter_contract.md)，[.codex/README.md](../.codex/README.md) |
| 插件 ABI / routing metadata | [runtime_plugin_contract.md](runtime_plugin_contract.md) |
| 历史迁移、减法记录 | [history/](history/) |

## 概念与源码映射

见 [harness_architecture.md §6](harness_architecture.md#6-与仓库文件的映射)。

## 已淘汰叙述（清理边界）

- **勿假设** `router-rs` 只存在于 `scripts/router-rs/target/release/`。根目录 `.cargo/config.toml` 可将 `target-dir` 指到 workspace 统一目录；解析以 `cargo metadata` 的 `target_directory` 为准（或 `cargo build` / `cargo run` 的输出路径）。
- **勿依赖** 旧版 `.cursor/hooks/*.sh` 脚本链：steady-state 以 [`.cursor/hooks.json`](../.cursor/hooks.json) 为准，事件 stdin 由各条目中的 `router-rs cursor hook --event=...` 承接（校验：在仓库根执行 `cargo run --manifest-path scripts/router-rs/Cargo.toml -- framework maint verify-cursor-hooks`）。
- **勿将** `docs/history/` 中的计划或清单当作当前契约；steady-state 仅认本索引列出的文档与 `configs/framework/*.json`。
