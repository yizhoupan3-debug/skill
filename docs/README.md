# 文档索引（控制面与契约）

**叙事分工**：仓库根 `AGENTS.md` = 跨宿主执行与语言策略；[`harness_architecture.md`](harness_architecture.md) = 连续性 **L1–L5 控制面**上层真源；[`rust_contracts.md`](rust_contracts.md)（英文）= `router-rs` 实现侧契约长文；[`history/`](history/) = 迁移与旧方案归档，**不作为当前运行真源**。

## 推荐阅读顺序

1. [仓库根 README.md](../README.md) — 分享、安装、Cursor/Codex hook 快速入门  
2. [AGENTS.md](../AGENTS.md) — Skill 路由、Continuity、Closeout、Execution Ladder  
3. [harness_architecture.md](harness_architecture.md) — 五层模型、证据流、续跑流、扩展规则（含 `HARNESS_OPERATOR_NUDGES`）  
4. [rust_contracts.md](rust_contracts.md) — 路由、profile、宿主集成、EVIDENCE_INDEX 等 Rust 业主  
5. [task_state_unified_resolve.md](task_state_unified_resolve.md) — `ResolvedTaskView` / `framework task-state-resolve`  

## 按主题

| 主题 | 文档 |
|------|------|
| Closeout 程序化门禁与 schema | [closeout_enforcement.md](closeout_enforcement.md)，`configs/framework/CLOSEOUT_RECORD_SCHEMA.json` |
| `framework_profile` 与默认面 | [framework_profile_contract.md](framework_profile_contract.md) |
| Codex 宿主投影边界 | [host_adapter_contracts.md](host_adapter_contracts.md)，[.codex/README.md](../.codex/README.md) |
| 插件 ABI / routing metadata | [runtime_plugin_contract.md](runtime_plugin_contract.md) |
| 历史迁移、减法记录 | [history/](history/) |

## 概念与源码映射

见 [harness_architecture.md §6](harness_architecture.md#6-与仓库文件的映射)。

## 已淘汰叙述（清理边界）

- **勿假设** `router-rs` 只存在于 `scripts/router-rs/target/release/`。根目录 `.cargo/config.toml` 可将 `target-dir` 指到 workspace 统一目录；解析以 `cargo metadata` 的 `target_directory` 为准，或直接使用 [`.cursor/hooks/resolve-router-rs.sh`](../.cursor/hooks/resolve-router-rs.sh)。  
- **勿依赖** `.cursor/hooks/legacy/` 作为 hook 回退；以 [`.cursor/hooks.json`](../.cursor/hooks.json) 与当前 `review-gate.sh`、`post-tool-use.sh` 等脚本为准。  
- **勿将** `docs/history/` 中的计划或清单当作当前契约；steady-state 仅认本索引列出的文档与 `configs/framework/*.json`。
