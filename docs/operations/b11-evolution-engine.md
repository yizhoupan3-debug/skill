---
last_verified: "2026-06-09"
plate: B11
---

# B11 — evolution-engine

## 职责

自进化离线分析：**零 crate 级依赖**于 B0/B1/B3；通过 mmap 读取遥测 JSONL。独立 crate：`core/evolution-rs/`。

数据管道：B0 **TelemetryWriter** → journal → `evolution-rs analyze|audit`。

## 启动 / 配置

```bash
# 配置真源
# configs/evolution/evolution.toml

cargo run --manifest-path core/evolution-rs/Cargo.toml -- analyze --help
cargo run --manifest-path core/evolution-rs/Cargo.toml -- audit --config configs/evolution/evolution.toml

# 环境变量
export EVOLUTION_RS_CONFIG=configs/evolution/evolution.toml
```

自动触发：`session_supervisor` 在 worker **idle** 时可 spawn `evolution-rs analyze`（dry-run / `force_evolution_idle` 路径，EV-5/EV-7）。

Hook 细粒度：`ROUTER_RS_HOOK_TIMING=1` → `HookFired` + timing 字段进 journal。

## 排障

| 现象 | 处理 |
|------|------|
| analyze 找不到 journal | 确认 telemetry 目录与 `evolution-rs-missing-journal` 提示路径 |
| RFV 分桶为空 | 需 `rfv_round` / `rfv_verdict_by_bucket` 事件已写入 |
| idle analyze 未触发 | 检查仍有 running worker；或 supervisor `list` 看 `evolution_idle` |
| 配置不生效 | `--config` 或 `EVOLUTION_RS_CONFIG` 指向 `evolution.toml` |

## 相关路径

- `core/evolution-rs/`
- `configs/evolution/evolution.toml`
- `artifacts/telemetry/`（events.jsonl 等，以 runtime 为准）
- `core/runtime-core/src/session_supervisor/evolution_idle.rs`
- `artifacts/current/roadmap-v5-exec/lane-notes/phase-ev7-*.json`
