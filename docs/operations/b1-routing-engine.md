---
last_verified: "2026-06-09"
plate: B1
---

# B1 — routing-engine

## 职责

查询分词、技能路由决策、评分权重与 **热路由** JSON 消费。独立 crate：`core/routing-engine/`。

与 B0 的边界：通过 **TokenizerProvider** trait 注入，避免 B0 物理依赖 B1。

## 启动 / 配置

| 资产 | 路径 |
|------|------|
| 热路由（运行时唯一入口） | `skills/SKILL_ROUTING_RUNTIME.json` |
| 冷元数据 | `skills/SKILL_MANIFEST.json`、`skills/SKILL_ROUTING_METADATA.json` |
| 路由诊断 | `cargo run --release --manifest-path core/router-rs/Cargo.toml -- route <query>` |

改热路由后：`framework skills validate` → `framework skills refresh`（或 `framework maint update-one-shot` 全量 drift-gate）。

## 排障

| 现象 | 处理 |
|------|------|
| 技能未命中 / 错路由 | `route <query>` 看 decision；核对 `SKILL_ROUTING_RUNTIME.json` slug |
| 并行 review 候选未触发 | `routing_signals` + registry `reviewer_lanes`；勿在 AGENTS 手写 lane 表 |
| 子串假阳性 | 见 roadmap P1 scoring 权重；`configs/` 下 scoring 集中化配置 |
| 路由测试红 | `cargo test -p router-rs routing_eval` / `search_regression` 簇 |

## 相关路径

- `core/routing-engine/`
- `core/runtime-core/src/route/`
- `skills/SKILL_ROUTING_RUNTIME.json`
- `docs/rust_contracts/index.md`（`route` stdio 所有权）
