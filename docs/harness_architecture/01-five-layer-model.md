---
last_verified: "2026-06-02"
depends_on:
  - ../host_adapter_contract.md
  - ../rust_contracts/index.md
  - ../task_state_unified_resolve.md
---

# 五层模型与结构

[返回索引](index.md)

## 1. 五层模型

```text
L5  Skill / RFV / orchestration contract
L4  Host projection (Cursor/Codex/Claude hooks)
L3  router-rs control plane
L2  Continuity artifacts under artifacts/current/
L1  Executable verification and exit codes
```

依赖方向只允许 `L1 -> L2 -> L3 -> L4 -> L5` 向上消费事实。L5 不得绕过 L2 自称"已完成"。

**术语**：上文 **L4 = 宿主 hook 投影**（Cursor/Codex/Claude）。`SKILL.md` frontmatter 里的 **`routing_layer: L4`** 表示 **冷表 manifest 技能**（如 `python-env-management`），与 harness 层号**不是同一概念**。

## 7. 扩展规则

1. 新宿主行为先判断属于哪条现有管道，再实现；不要在 L4 脚本复制 L3 逻辑。
2. 新环境变量只在确实改变行为边界时添加；默认合并分支而不是继续加旋钮。
3. 新 operator 文案默认写进配置或文档，不写进零散 `const`。
4. 新验证启发式必须有测试；宁可少而准。
5. 改动 SessionStart 或 routing 热路径时，先证明 token 预算更小、真源更少，而不是只换说法。

## 8. 文件映射

| 概念 | 主要落地 |
|------|----------|
| L4 hooks | `.cursor/hooks.json`、`.codex/hooks.json`、各宿主 hook 配置 |
| L3 control plane | `core/runtime-core/src/`：`hosts/codex_hooks/mod.rs`、`claude_code_hooks.rs`、`cursor_hooks/mod.rs`（`handlers.rs` + `handlers_parts/*.inc.rs`）、`autopilot_goal.rs`、`rfv_loop.rs`、`framework_runtime/mod.rs`、`task_state.rs`、`host_integration/mod.rs` |
| L2 continuity | `artifacts/current/`、`TRACE_EVENTS.jsonl`、`STEP_LEDGER.jsonl`、`configs/framework/*SCHEMA*` |
| Skill 热路由（router-rs hot path） | `skills/SKILL_ROUTING_RUNTIME.json` |
| Skill 伴生元数据（**非**每 prompt 热路径；`SKILL_PLUGIN_CATALOG` / `SKILL_ROUTING_RUNTIME_EXPLAIN` 由 refresh / policy / CI 消费；**`SKILL_ROUTING_METADATA.json` 在 `load_records_from_runtime` 时 merge**，见 `route/records.rs` `merge_sidecar_route_metadata_from_runtime`） | `skills/SKILL_PLUGIN_CATALOG.json`、`skills/SKILL_ROUTING_METADATA.json`、`skills/SKILL_ROUTING_RUNTIME_EXPLAIN.json`（EXPLAIN：CI/companion/人读，router route 模块不读） |
| Host registry（磁盘 loader） | `configs/framework/RUNTIME_REGISTRY.json` + `core/runtime-core/src/runtime_registry/mod.rs`（shim：``） |
| 宿主投影 My/review 文案 | `configs/framework/host_projection_narrative.json` |
| 生成物 manifest / drift | `configs/framework/GENERATED_ARTIFACTS.json` + `framework host-integration generated-artifacts-status` |
| 任务 schema drift / Cursor hooks 减法闭集 | `core/runtime-core/src/schema_drift.rs`、`hosts/cursor_hooks/subtraction.rs`；CLI `router-rs schema-drift {contract,baseline,check}` |
| 弱模型 / 上下文预算调研索引 | 见 `skills/SKILL_ROUTING_RUNTIME.json` 的 hot/cold 分布，或运行 `router-rs eval route` |
| 全面自检清单（减法审计，非合并门槛） | 运行 `router-rs framework maint update-audit --repo-root .` |

## 9. 刻意不做的事

- 不在 SessionStart 注入 repo onboarding。
- 不保留旧 runtime shape 兼容层。
- 不在 `AGENTS.md`、Cursor rules、docs、hook 文案里重复展开同一套长叙事。
- 不为了"也许以后需要"保留 verbose 模式、双通道切换或多事件重复续跑注入。
