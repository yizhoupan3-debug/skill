# 文档索引（控制面与契约）

**叙事分工**：仓库根 `AGENTS.md` = 跨宿主执行与语言策略；[`harness_architecture.md`](harness_architecture.md) = 连续性 **L1–L5 控制面**上层真源；[`rust_contracts.md`](rust_contracts.md)（英文）= `router-rs` 实现侧契约长文；历史迁移叙述见 git 历史与 [`MIGRATION.md`](../MIGRATION.md)。

## 推荐阅读顺序

1. [仓库根 README.md](../README.md) — 分享、安装、Cursor/Codex hook 快速入门  
2. [framework_operator_primer.md](framework_operator_primer.md) — 使用者一页纸：宿主差异、`REVIEW_GATE` 快查、真源阅读顺序、自检 `framework doctor`  
3. [AGENTS.md](../AGENTS.md) — Skill 路由、Continuity、Closeout、Execution Ladder  
4. [harness_architecture.md](harness_architecture.md) — 五层模型、证据流、续跑（stdio + 手动画板，非 hook `GOAL_CONTINUE`）、扩展规则（含 `HARNESS_OPERATOR_NUDGES`）  
5. [rust_contracts.md](rust_contracts.md) — 路由、profile、宿主集成、EVIDENCE_INDEX 等 Rust 业主  
6. [task_state_unified_resolve.md](task_state_unified_resolve.md) — `ResolvedTaskView` / `framework task-state-resolve`  

## 按主题

| 主题 | 文档 |
|------|------|
| 使用者视角：宿主差异、门控快查、阅读顺序 | [framework_operator_primer.md](framework_operator_primer.md) |
| Env 命名与默认值表 | [framework_naming_conventions.md](framework_naming_conventions.md) |
| 政策分层地图（operator profiles 依赖） | [harness_policy_map.md](harness_policy_map.md) |
| RFV 多轮账本（`framework_rfv_loop`）契约与 lane 模板；数理推理强度 | [rfv_loop_harness.md](rfv_loop_harness.md)，[references/rfv-loop/](references/rfv-loop/)（含 [math-reasoning-harness.md](references/rfv-loop/math-reasoning-harness.md)） |
| 弱模型 / 上下文预算、Token 注入路径与 harness 合成交付 | 任务 ROADMAP：`artifacts/current/<task_id>/ROADMAP.md`；见 [plans/README.md](plans/README.md) |
| Closeout 程序化门禁与 schema | [closeout_enforcement.md](closeout_enforcement.md)，`configs/framework/CLOSEOUT_RECORD_SCHEMA.json` |
| `framework_profile` 与默认面 | [framework_profile_contract.md](framework_profile_contract.md) |
| 新宿主接入 / 多宿主适配 | [§3.1 工程清单](host_adapter_contract.md#31-可复制执行清单工程顺序)（文首 **快速路径** 同文件）；`RUNTIME_REGISTRY`、`registry_loader`、`host_projection_narrative`、`GENERATED_ARTIFACTS` 见 [harness_architecture.md §2.3](harness_architecture.md#23-控制面配置与生成物2026-05-20-硬化)；多宿主 harness 契约见唯一真源 [host_adapter_contract.md](host_adapter_contract.md) |
| 生成物 drift / doctor 快探针 | [harness_architecture.md §2.3](harness_architecture.md#23-控制面配置与生成物2026-05-20-硬化)；`framework host-integration generated-artifacts-status [--skip-generator-run]` |
| 任务级 schema drift（hooks 7 事件闭集、模板 parity、REQUIREMENTS↔ROADMAP 标题） | `router-rs schema-drift contract` / `baseline` / `check`（[`schema_drift.rs`](../scripts/router-rs/src/schema_drift.rs)）；验收见 [`skills/verifyx/SKILL.md`](../skills/verifyx/SKILL.md)、[`configs/framework/SCHEMA_DRIFT_HEADINGS_CONTRACT.md`](../configs/framework/SCHEMA_DRIFT_HEADINGS_CONTRACT.md) |
| Cursor Plan / My 可验收 todo | [`skills/plan-mode/SKILL.md`](../skills/plan-mode/SKILL.md)、[`skills/planx/SKILL.md`](../skills/planx/SKILL.md)；[`.cursor/rules/cursor-plan-output.mdc`](../.cursor/rules/cursor-plan-output.mdc)；索引 [plans/README.md](plans/README.md) |
| Codex 宿主投影边界 | [host_adapter_contract.md](host_adapter_contract.md)，[.codex/README.md](../.codex/README.md) |
| 运行期核心行为与沙箱统一规约 | [runtime_unified_spec.md](runtime_unified_spec.md) |
| Python 环境治理（uv-only，冷表显式 `$python-env-management`） | [`skills/python-env-management/SKILL.md`](../skills/.archive-cold/python-env-management/SKILL.md) |
| 历史迁移、减法记录 | [`MIGRATION.md`](../MIGRATION.md)、git 历史 |
| Plans 索引（ROADMAP 真源；已删 stub 不恢复） | [plans/README.md](plans/README.md) |

## 概念与源码映射

见 [harness_architecture.md §6](harness_architecture.md#6-与仓库文件的映射)。

## 已淘汰叙述（清理边界）

- **勿假设** `router-rs` 只存在于 `scripts/router-rs/target/release/`。根目录 `.cargo/config.toml` 可将 `target-dir` 指到 workspace 统一目录；解析以 `cargo metadata` 的 `target_directory` 为准（或 `cargo build` / `cargo run` 的输出路径）。
- **勿依赖** 旧版 `.cursor/hooks/*.sh` 脚本链：steady-state 以 [`.cursor/hooks.json`](../.cursor/hooks.json) 为准（**默认 7 事件**；见 [`docs/hosts/cursor.md`](hosts/cursor.md)）。Claude Code 为 [`.claude/settings.json`](../.claude/settings.json) **4 事件**（见 [`docs/hosts/claude.md`](hosts/claude.md)）。校验：`framework maint verify-cursor-hooks`；构建 release 见两宿主手册「内存 / release」。
- **勿将** 已删除的 `docs/history/` 或过期 plan 路径当作当前契约；steady-state 仅认本索引列出的文档与 `configs/framework/*.json`。
